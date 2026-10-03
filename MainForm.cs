using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using System.Web.Script.Serialization;
using System.Windows.Forms;

namespace WinGlow
{
    public sealed class MainForm : Form
    {
        private readonly FontTunerService _service;
        private readonly UpdateService _updateService;
        private ComboBox _presetComboBox;
        private Label _descriptionLabel;
        private Label _fontHintLabel;
        private Label _packageInfoLabel;
        private Label _adminLabel;
        private Label _versionBadgeLabel;
        private Label _updateStatusLabel;
        private Label _heroSubtitleLabel;
        private CheckBox _rebuildCacheCheckBox;
        private CheckBox _restartExplorerCheckBox;
        private TextBox _logTextBox;
        private ModernButton _installFontsButton;
        private ModernButton _openUpdateButton;
        private ModernButton _checkUpdatesButton;
        private ModernButton _openDownloadButton;
        private ModernButton _clearLogButton;
        private Label _quickPickLabel;
        private List<FontPreset> _presets;
        private Dictionary<string, FontPackage> _fontPackages;
        private UpdateCheckResult _latestUpdateResult;

        public MainForm()
        {
            _service = new FontTunerService();
            _updateService = new UpdateService();
            _presets = new List<FontPreset>();
            _fontPackages = new Dictionary<string, FontPackage>(StringComparer.OrdinalIgnoreCase);

            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint, true);
            DoubleBuffered = true;
            AutoScaleMode = AutoScaleMode.Dpi;
            Text = "WinGlow";
            Width = 1180;
            Height = 920;
            MinimumSize = new Size(1040, 760);
            StartPosition = FormStartPosition.CenterScreen;
            BackColor = UiPalette.WindowBackground;
            Font = new Font(SystemFonts.MessageBoxFont.FontFamily, 9.25f, FontStyle.Regular);
            Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath);

            TableLayoutPanel shell = new TableLayoutPanel();
            shell.Dock = DockStyle.Fill;
            shell.Padding = new Padding(20);
            shell.BackColor = Color.Transparent;
            shell.ColumnCount = 1;
            shell.RowCount = 3;
            shell.RowStyles.Add(new RowStyle(SizeType.Absolute, 152f));
            shell.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
            shell.RowStyles.Add(new RowStyle(SizeType.Absolute, 108f));
            Controls.Add(shell);

            shell.Controls.Add(BuildHeroCard(), 0, 0);
            shell.Controls.Add(BuildBodySection(), 0, 1);
            shell.Controls.Add(BuildLogCard(), 0, 2);

            Load += MainForm_Load;
            Shown += MainForm_Shown;
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            TryApplyWindowEffects();
        }

        protected override void OnPaintBackground(PaintEventArgs e)
        {
            Rectangle bounds = ClientRectangle;

            using (LinearGradientBrush brush = new LinearGradientBrush(
                bounds,
                Color.FromArgb(242, 247, 252),
                Color.FromArgb(232, 240, 249),
                90f))
            {
                e.Graphics.FillRectangle(brush, bounds);
            }

            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;

            using (SolidBrush accentBrush = new SolidBrush(Color.FromArgb(28, 27, 108, 236)))
            using (SolidBrush warmBrush = new SolidBrush(Color.FromArgb(20, 114, 197, 255)))
            {
                e.Graphics.FillEllipse(accentBrush, new Rectangle(Width - 300, -90, 360, 360));
                e.Graphics.FillEllipse(warmBrush, new Rectangle(-140, Height - 280, 280, 280));
            }
        }

        private Control BuildHeroCard()
        {
            ModernCardPanel hero = new ModernCardPanel();
            hero.Dock = DockStyle.Fill;
            hero.UseGradient = true;
            hero.ShowGlow = true;
            hero.CornerRadius = 34;
            hero.Padding = new Padding(28, 18, 28, 18);
            hero.Margin = new Padding(0, 0, 0, 12);

            TableLayoutPanel layout = new TableLayoutPanel();
            layout.Dock = DockStyle.Fill;
            layout.BackColor = Color.Transparent;
            layout.ColumnCount = 2;
            layout.RowCount = 1;
            layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 56f));
            layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 44f));
            hero.Controls.Add(layout);

            Panel left = new Panel();
            left.Dock = DockStyle.Fill;
            left.BackColor = Color.Transparent;
            layout.Controls.Add(left, 0, 0);

            Label title = new Label();
            title.AutoSize = true;
            title.BackColor = Color.Transparent;
            title.Font = new Font(Font.FontFamily, 18.5f, FontStyle.Bold);
            title.ForeColor = UiPalette.TextPrimary;
            title.Text = "把 Windows 字体调成更舒服的样子";
            title.MaximumSize = new Size(540, 0);
            title.Location = new Point(0, 2);
            title.Text = Text;
            left.Controls.Add(title);

            _heroSubtitleLabel = new Label();
            _heroSubtitleLabel.AutoSize = false;
            _heroSubtitleLabel.BackColor = Color.Transparent;
            _heroSubtitleLabel.ForeColor = UiPalette.TextSecondary;
            _heroSubtitleLabel.Font = new Font(Font.FontFamily, 10f, FontStyle.Regular);
            _heroSubtitleLabel.Location = new Point(0, 42);
            _heroSubtitleLabel.Size = new Size(560, 46);
            _heroSubtitleLabel.Visible = false;
            _heroSubtitleLabel.Text =
                "内置三套可直接安装的字体包，也支持一键备份、恢复、应用预设。" + Environment.NewLine +
                "这一版还加了更圆润的界面和本地检查更新，别人装完软件后也能看到新版本提示。";
            left.Controls.Add(_heroSubtitleLabel);

            _versionBadgeLabel = new Label();
            _versionBadgeLabel.AutoSize = true;
            _versionBadgeLabel.BackColor = UiPalette.AccentSoft;
            _versionBadgeLabel.ForeColor = UiPalette.Accent;
            _versionBadgeLabel.Font = new Font(Font.FontFamily, 9f, FontStyle.Bold);
            _versionBadgeLabel.Padding = new Padding(12, 5, 12, 5);
            _versionBadgeLabel.Location = new Point(0, 56);
            left.Controls.Add(_versionBadgeLabel);

            Panel right = new Panel();
            right.Dock = DockStyle.Fill;
            right.BackColor = Color.Transparent;
            layout.Controls.Add(right, 1, 0);

            Label updateTitle = new Label();
            updateTitle.AutoSize = true;
            updateTitle.BackColor = Color.Transparent;
            updateTitle.Font = new Font(Font.FontFamily, 10.5f, FontStyle.Bold);
            updateTitle.ForeColor = UiPalette.TextPrimary;
            updateTitle.Text = "版本与更新";
            updateTitle.Location = new Point(0, 0);
            right.Controls.Add(updateTitle);

            _updateStatusLabel = new Label();
            _updateStatusLabel.AutoSize = false;
            _updateStatusLabel.BackColor = Color.Transparent;
            _updateStatusLabel.ForeColor = UiPalette.TextSecondary;
            _updateStatusLabel.Location = new Point(0, 28);
            _updateStatusLabel.Size = new Size(360, 36);
            _updateStatusLabel.Text = "启动后会自动检查 GitHub Release，也可以手动点按钮立即检查。";
            right.Controls.Add(_updateStatusLabel);

            FlowLayoutPanel actions = new FlowLayoutPanel();
            actions.AutoSize = false;
            actions.BackColor = Color.Transparent;
            actions.WrapContents = false;
            actions.Size = new Size(390, 40);
            actions.Location = new Point(0, 66);
            actions.Margin = new Padding(0);
            right.Controls.Add(actions);

            _checkUpdatesButton = BuildButton("检查更新", CheckUpdatesButton_Click, ModernButtonStyle.Primary, 126);
            actions.Controls.Add(_checkUpdatesButton);

            _openUpdateButton = BuildButton("打开项目主页", OpenUpdatePageButton_Click, ModernButtonStyle.Secondary, 138);
            actions.Controls.Add(_openUpdateButton);

            _openDownloadButton = BuildButton("下载新版本", OpenDownloadButton_Click, ModernButtonStyle.Secondary, 138);
            _openDownloadButton.Visible = false;
            actions.Controls.Add(_openDownloadButton);
            _checkUpdatesButton.Width = 110;
            _openUpdateButton.Width = 122;
            _openDownloadButton.Width = 118;

            return hero;
        }

        private Control BuildBodySection()
        {
            TableLayoutPanel body = new TableLayoutPanel();
            body.Dock = DockStyle.Fill;
            body.BackColor = Color.Transparent;
            body.ColumnCount = 2;
            body.RowCount = 1;
            body.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 58f));
            body.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 42f));
            body.Margin = new Padding(0, 0, 0, 8);

            body.Controls.Add(BuildPresetCard(), 0, 0);
            body.Controls.Add(BuildStatusCard(), 1, 0);

            return body;
        }

        private Control BuildPresetCard()
        {
            ModernCardPanel card = new ModernCardPanel();
            card.Dock = DockStyle.Fill;
            card.CornerRadius = 28;
            card.Padding = new Padding(22);
            card.Margin = new Padding(0, 0, 10, 0);

            TableLayoutPanel layout = new TableLayoutPanel();
            layout.Dock = DockStyle.Fill;
            layout.BackColor = Color.Transparent;
            layout.ColumnCount = 1;
            layout.RowCount = 6;
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
            card.Controls.Add(layout);

            Label title = new Label();
            title.AutoSize = true;
            title.BackColor = Color.Transparent;
            title.Font = new Font(Font.FontFamily, 12.5f, FontStyle.Bold);
            title.ForeColor = UiPalette.TextPrimary;
            title.Text = "预设与操作";
            layout.Controls.Add(title, 0, 0);

            Label sub = new Label();
            sub.AutoSize = true;
            sub.BackColor = Color.Transparent;
            sub.Margin = new Padding(0, 6, 0, 0);
            sub.ForeColor = UiPalette.TextMuted;
            sub.Text = "建议顺序：先安装字体，再应用预设。";
            layout.Controls.Add(sub, 0, 1);

            Panel pickerRow = new Panel();
            pickerRow.Height = 52;
            pickerRow.Dock = DockStyle.Top;
            pickerRow.BackColor = Color.Transparent;
            pickerRow.Margin = new Padding(0, 18, 0, 0);
            layout.Controls.Add(pickerRow, 0, 2);

            Label presetLabel = new Label();
            presetLabel.AutoSize = true;
            presetLabel.BackColor = Color.Transparent;
            presetLabel.Text = "预设";
            presetLabel.ForeColor = UiPalette.TextSecondary;
            presetLabel.Location = new Point(0, 14);
            pickerRow.Controls.Add(presetLabel);

            _presetComboBox = new ComboBox();
            _presetComboBox.DropDownStyle = ComboBoxStyle.DropDownList;
            _presetComboBox.FlatStyle = FlatStyle.Flat;
            _presetComboBox.Width = 340;
            _presetComboBox.Height = 32;
            _presetComboBox.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            _presetComboBox.Location = new Point(58, 9);
            _presetComboBox.SelectedIndexChanged += PresetComboBox_SelectedIndexChanged;
            pickerRow.Controls.Add(_presetComboBox);

            FlowLayoutPanel actions = new FlowLayoutPanel();
            actions.AutoSize = true;
            actions.BackColor = Color.Transparent;
            actions.WrapContents = true;
            actions.Margin = new Padding(0, 10, 0, 0);
            layout.Controls.Add(actions, 0, 3);

            actions.Controls.Add(BuildButton("应用当前预设", ApplyButton_Click, ModernButtonStyle.Primary, 138));
            _installFontsButton = BuildButton("安装所需字体", InstallFontsButton_Click, ModernButtonStyle.Secondary, 118);
            actions.Controls.Add(_installFontsButton);
            _installFontsButton.Text = "安装所需字体";
            _installFontsButton.Width = 118;
            actions.Controls.Add(BuildButton("创建备份", BackupButton_Click, ModernButtonStyle.Secondary, 116));
            actions.Controls.Add(BuildButton("恢复最近备份", RestoreButton_Click, ModernButtonStyle.Secondary, 142));
            actions.Controls.Add(BuildButton("打开备份目录", OpenBackupsButton_Click, ModernButtonStyle.Ghost, 128));
            actions.Controls.Add(BuildButton("打开预设目录", OpenPresetsButton_Click, ModernButtonStyle.Ghost, 128));

            ModernCardPanel descriptionCard = new ModernCardPanel();
            descriptionCard.Dock = DockStyle.Top;
            descriptionCard.CornerRadius = 22;
            descriptionCard.FillColor = UiPalette.Secondary;
            descriptionCard.BorderColor = Color.FromArgb(224, 231, 241);
            descriptionCard.Padding = new Padding(16);
            descriptionCard.Margin = new Padding(0, 16, 0, 0);
            descriptionCard.Height = 120;
            layout.Controls.Add(descriptionCard, 0, 4);

            Label descriptionTitle = new Label();
            descriptionTitle.AutoSize = true;
            descriptionTitle.BackColor = Color.Transparent;
            descriptionTitle.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            descriptionTitle.ForeColor = UiPalette.TextPrimary;
            descriptionTitle.Text = "当前预设说明";
            descriptionTitle.Location = new Point(16, 14);
            descriptionCard.Controls.Add(descriptionTitle);

            _descriptionLabel = new Label();
            _descriptionLabel.AutoSize = false;
            _descriptionLabel.BackColor = Color.Transparent;
            _descriptionLabel.ForeColor = UiPalette.TextSecondary;
            _descriptionLabel.Location = new Point(16, 40);
            _descriptionLabel.Size = new Size(520, 72);
            _descriptionLabel.Text = "加载中...";
            descriptionCard.Controls.Add(_descriptionLabel);

            ModernCardPanel quickPickCard = new ModernCardPanel();
            quickPickCard.Dock = DockStyle.Fill;
            quickPickCard.CornerRadius = 22;
            quickPickCard.FillColor = Color.FromArgb(228, 246, 242, 249);
            quickPickCard.BorderColor = Color.FromArgb(222, 231, 240);
            quickPickCard.Padding = new Padding(16);
            quickPickCard.Margin = new Padding(0, 16, 0, 0);
            quickPickCard.Visible = false;
            layout.Controls.Add(quickPickCard, 0, 5);

            Label quickPickTitle = new Label();
            quickPickTitle.AutoSize = true;
            quickPickTitle.BackColor = Color.Transparent;
            quickPickTitle.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            quickPickTitle.ForeColor = UiPalette.TextPrimary;
            quickPickTitle.Text = "快速建议";
            quickPickTitle.Location = new Point(16, 14);
            quickPickCard.Controls.Add(quickPickTitle);

            _quickPickLabel = new Label();
            _quickPickLabel.AutoSize = false;
            _quickPickLabel.BackColor = Color.Transparent;
            _quickPickLabel.ForeColor = UiPalette.TextSecondary;
            _quickPickLabel.Location = new Point(16, 40);
            _quickPickLabel.Size = new Size(520, 92);
            _quickPickLabel.Text =
                "HarmonyOS：整体更现代，适合想要统一感的人。" + Environment.NewLine +
                "思源黑体：最稳妥，适合作为长期默认方案。" + Environment.NewLine +
                "更纱 UI：更有存在感，适合觉得 Windows 默认太细的人。";
            quickPickCard.Controls.Add(_quickPickLabel);

            return card;
        }

        private Control BuildStatusCard()
        {
            ModernCardPanel card = new ModernCardPanel();
            card.Dock = DockStyle.Fill;
            card.CornerRadius = 28;
            card.Padding = new Padding(22);
            card.Margin = new Padding(10, 0, 0, 0);

            TableLayoutPanel layout = new TableLayoutPanel();
            layout.Dock = DockStyle.Fill;
            layout.BackColor = Color.Transparent;
            layout.ColumnCount = 1;
            layout.RowCount = 6;
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            card.Controls.Add(layout);

            Label title = new Label();
            title.AutoSize = true;
            title.BackColor = Color.Transparent;
            title.Font = new Font(Font.FontFamily, 12.5f, FontStyle.Bold);
            title.ForeColor = UiPalette.TextPrimary;
            title.Text = "状态与资源";
            layout.Controls.Add(title, 0, 0);

            Panel adminHost = new Panel();
            adminHost.Dock = DockStyle.Top;
            adminHost.Height = 44;
            adminHost.Margin = new Padding(0, 10, 0, 0);
            adminHost.BackColor = Color.Transparent;
            layout.Controls.Add(adminHost, 0, 1);

            _adminLabel = new Label();
            _adminLabel.AutoSize = false;
            _adminLabel.Dock = DockStyle.Fill;
            _adminLabel.TextAlign = ContentAlignment.TopLeft;
            _adminLabel.BackColor = Color.Transparent;
            _adminLabel.Font = new Font(Font.FontFamily, 9f, FontStyle.Bold);
            adminHost.Controls.Add(_adminLabel);

            FlowLayoutPanel options = new FlowLayoutPanel();
            options.AutoSize = true;
            options.BackColor = Color.Transparent;
            options.WrapContents = true;
            options.Margin = new Padding(0, 14, 0, 0);
            layout.Controls.Add(options, 0, 2);

            _rebuildCacheCheckBox = new CheckBox();
            _rebuildCacheCheckBox.AutoSize = true;
            _rebuildCacheCheckBox.BackColor = Color.Transparent;
            _rebuildCacheCheckBox.Checked = true;
            _rebuildCacheCheckBox.ForeColor = UiPalette.TextSecondary;
            _rebuildCacheCheckBox.Text = "重建字体缓存";
            options.Controls.Add(_rebuildCacheCheckBox);

            _restartExplorerCheckBox = new CheckBox();
            _restartExplorerCheckBox.AutoSize = true;
            _restartExplorerCheckBox.BackColor = Color.Transparent;
            _restartExplorerCheckBox.Checked = true;
            _restartExplorerCheckBox.ForeColor = UiPalette.TextSecondary;
            _restartExplorerCheckBox.Text = "应用后重启资源管理器";
            options.Controls.Add(_restartExplorerCheckBox);

            ModernCardPanel fontCard = new ModernCardPanel();
            fontCard.Dock = DockStyle.Top;
            fontCard.CornerRadius = 22;
            fontCard.FillColor = UiPalette.Secondary;
            fontCard.BorderColor = Color.FromArgb(224, 231, 241);
            fontCard.Padding = new Padding(16);
            fontCard.Margin = new Padding(0, 16, 0, 0);
            fontCard.Height = 88;
            layout.Controls.Add(fontCard, 0, 3);

            Label fontTitle = new Label();
            fontTitle.AutoSize = true;
            fontTitle.BackColor = Color.Transparent;
            fontTitle.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            fontTitle.ForeColor = UiPalette.TextPrimary;
            fontTitle.Text = "字体状态";
            fontTitle.Location = new Point(16, 14);
            fontCard.Controls.Add(fontTitle);

            _fontHintLabel = new Label();
            _fontHintLabel.AutoSize = false;
            _fontHintLabel.BackColor = Color.Transparent;
            _fontHintLabel.ForeColor = UiPalette.TextSecondary;
            _fontHintLabel.Location = new Point(16, 40);
            _fontHintLabel.Size = new Size(360, 42);
            fontCard.Controls.Add(_fontHintLabel);

            ModernCardPanel packageCard = new ModernCardPanel();
            packageCard.Dock = DockStyle.Fill;
            packageCard.CornerRadius = 22;
            packageCard.FillColor = UiPalette.Secondary;
            packageCard.BorderColor = Color.FromArgb(224, 231, 241);
            packageCard.Padding = new Padding(16);
            packageCard.Margin = new Padding(0, 16, 0, 0);
            layout.Controls.Add(packageCard, 0, 4);

            Label packageTitle = new Label();
            packageTitle.AutoSize = true;
            packageTitle.BackColor = Color.Transparent;
            packageTitle.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            packageTitle.ForeColor = UiPalette.TextPrimary;
            packageTitle.Text = "内置字体包";
            packageTitle.Location = new Point(16, 14);
            packageCard.Controls.Add(packageTitle);

            _packageInfoLabel = new Label();
            _packageInfoLabel.AutoSize = false;
            _packageInfoLabel.BackColor = Color.Transparent;
            _packageInfoLabel.ForeColor = UiPalette.TextSecondary;
            _packageInfoLabel.Location = new Point(16, 40);
            _packageInfoLabel.Size = new Size(360, 188);
            packageCard.Controls.Add(_packageInfoLabel);

            FlowLayoutPanel resourceActions = new FlowLayoutPanel();
            resourceActions.AutoSize = true;
            resourceActions.BackColor = Color.Transparent;
            resourceActions.WrapContents = true;
            resourceActions.Margin = new Padding(0, 16, 0, 0);
            layout.Controls.Add(resourceActions, 0, 5);

            resourceActions.Controls.Add(BuildButton("字体包目录", OpenFontPackagesButton_Click, ModernButtonStyle.Ghost, 98));
            resourceActions.Controls.Add(BuildButton("GitHub 发布页", OpenReleasePageButton_Click, ModernButtonStyle.Ghost, 112));

            return card;
        }

        private Control BuildLogCard()
        {
            ModernCardPanel card = new ModernCardPanel();
            card.Dock = DockStyle.Fill;
            card.CornerRadius = 28;
            card.Padding = new Padding(22);
            card.Margin = new Padding(0);

            TableLayoutPanel layout = new TableLayoutPanel();
            layout.Dock = DockStyle.Fill;
            layout.BackColor = Color.Transparent;
            layout.ColumnCount = 1;
            layout.RowCount = 2;
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
            card.Controls.Add(layout);

            TableLayoutPanel header = new TableLayoutPanel();
            header.Dock = DockStyle.Top;
            header.BackColor = Color.Transparent;
            header.ColumnCount = 2;
            header.RowCount = 1;
            header.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f));
            header.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
            layout.Controls.Add(header, 0, 0);

            Panel titles = new Panel();
            titles.Dock = DockStyle.Fill;
            titles.BackColor = Color.Transparent;
            header.Controls.Add(titles, 0, 0);

            Label title = new Label();
            title.AutoSize = true;
            title.BackColor = Color.Transparent;
            title.Font = new Font(Font.FontFamily, 12.5f, FontStyle.Bold);
            title.ForeColor = UiPalette.TextPrimary;
            title.Text = "操作日志";
            title.Location = new Point(0, 0);
            titles.Controls.Add(title);

            Label sub = new Label();
            sub.AutoSize = true;
            sub.BackColor = Color.Transparent;
            sub.ForeColor = UiPalette.TextMuted;
            sub.Location = new Point(0, 28);
            sub.Text = "备份、安装、应用预设和更新检查的结果都会在这里显示。";
            titles.Controls.Add(sub);

            _clearLogButton = BuildButton("清空日志", ClearLogButton_Click, ModernButtonStyle.Ghost, 104);
            _clearLogButton.Margin = new Padding(0);
            header.Controls.Add(_clearLogButton, 1, 0);

            ModernCardPanel logSurface = new ModernCardPanel();
            logSurface.Dock = DockStyle.Fill;
            logSurface.CornerRadius = 22;
            logSurface.FillColor = Color.FromArgb(242, 248, 252);
            logSurface.BorderColor = Color.FromArgb(224, 231, 241);
            logSurface.Padding = new Padding(14);
            logSurface.Margin = new Padding(0, 16, 0, 0);
            layout.Controls.Add(logSurface, 0, 1);

            _logTextBox = new TextBox();
            _logTextBox.Multiline = true;
            _logTextBox.ScrollBars = ScrollBars.Vertical;
            _logTextBox.ReadOnly = true;
            _logTextBox.BorderStyle = BorderStyle.None;
            _logTextBox.Dock = DockStyle.Fill;
            _logTextBox.BackColor = Color.FromArgb(242, 248, 252);
            _logTextBox.ForeColor = UiPalette.TextPrimary;
            _logTextBox.Font = new Font("Consolas", 9.4f, FontStyle.Regular);
            logSurface.Controls.Add(_logTextBox);

            return card;
        }

        private static ModernButton BuildButton(string text, EventHandler onClick, ModernButtonStyle style, int width)
        {
            ModernButton button = new ModernButton();
            button.Text = text;
            button.Width = width;
            button.ButtonStyle = style;
            button.CornerRadius = 12;
            button.Margin = new Padding(0, 0, 8, 8);
            button.Click += onClick;
            return button;
        }

        private void MainForm_Load(object sender, EventArgs e)
        {
            bool isAdmin = _service.IsAdministrator();
            _adminLabel.Text = isAdmin
                ? "管理员模式已启用，可以直接备份、应用和恢复系统字体设置。"
                : "当前不是管理员模式。要真正写入系统字体设置，请右键“以管理员身份运行”。";
            _adminLabel.Text = isAdmin
                ? "管理员模式已启用，可直接备份、应用和恢复设置。"
                : "当前不是管理员模式。要写入系统字体，请右键“以管理员身份运行”。";
            _adminLabel.ForeColor = isAdmin ? UiPalette.Success : UiPalette.Warning;

            _versionBadgeLabel.Text = "当前版本 v" + FormatVersion(GetCurrentVersion());

            LoadFontPackages();
            LoadPresets();
            RefreshPresetDetails();

            Log("备份目录：" + _service.BackupRoot);
            Log("当前版本已内置三套字体包，可先安装字体，再应用预设。");
            Log("启动后会自动检查 GitHub Release，用户也可以手动点“检查更新”。");
        }

        private async void MainForm_Shown(object sender, EventArgs e)
        {
            await CheckForUpdatesAsync(true);
        }

        private void LoadFontPackages()
        {
            string catalogPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "FontPackages", "catalog.json");
            JavaScriptSerializer serializer = new JavaScriptSerializer();
            List<FontPackage> packages = new List<FontPackage>();

            if (File.Exists(catalogPath))
            {
                packages = serializer.Deserialize<List<FontPackage>>(File.ReadAllText(catalogPath));
            }

            _fontPackages = packages
                .Where(function => function != null && !string.IsNullOrWhiteSpace(function.Id))
                .ToDictionary(function => function.Id, StringComparer.OrdinalIgnoreCase);
        }

        private void LoadPresets()
        {
            string presetDirectory = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Presets");
            List<FontPreset> presets = new List<FontPreset>();
            JavaScriptSerializer serializer = new JavaScriptSerializer();

            if (Directory.Exists(presetDirectory))
            {
                foreach (string file in Directory.GetFiles(presetDirectory, "*.json"))
                {
                    FontPreset preset = serializer.Deserialize<FontPreset>(File.ReadAllText(file));
                    if (preset == null)
                    {
                        continue;
                    }

                    preset.SourcePath = file;
                    presets.Add(preset);
                }
            }

            _presets = presets.OrderBy(function => function.Name).ToList();
            _presetComboBox.Items.Clear();

            foreach (FontPreset preset in _presets)
            {
                _presetComboBox.Items.Add(preset);
            }

            if (_presetComboBox.Items.Count > 0)
            {
                _presetComboBox.SelectedIndex = 0;
            }
        }

        private FontPreset GetSelectedPreset()
        {
            return _presetComboBox.SelectedItem as FontPreset;
        }

        private FontPackage GetSelectedFontPackage()
        {
            FontPreset preset = GetSelectedPreset();
            FontPackage package;

            if (preset == null || string.IsNullOrWhiteSpace(preset.FontPackageId))
            {
                return null;
            }

            return _fontPackages.TryGetValue(preset.FontPackageId, out package) ? package : null;
        }

        private void PresetComboBox_SelectedIndexChanged(object sender, EventArgs e)
        {
            RefreshPresetDetails();
        }

        private void RefreshPresetDetails()
        {
            FontPreset preset = GetSelectedPreset();

            if (preset == null)
            {
                _descriptionLabel.Text = "还没有读取到任何预设。";
                _fontHintLabel.Text = "未选择预设。";
                _packageInfoLabel.Text = "暂无内置字体包信息。";
                _installFontsButton.Enabled = false;
                return;
            }

            _descriptionLabel.Text = string.IsNullOrWhiteSpace(preset.Description)
                ? "这个预设暂时没有额外说明。"
                : preset.Description;

            IList<string> missing = _service.GetMissingFonts(preset);
            if (missing.Count == 0)
            {
                _fontHintLabel.Text = "所需字体已就绪：" + string.Join("、", preset.RequiredFonts ?? new List<string>());
            }
            else
            {
                _fontHintLabel.Text = "还缺这些字体：" + string.Join("、", missing);
            }

            FontPackage package = GetSelectedFontPackage();
            if (package == null)
            {
                _packageInfoLabel.Text = "当前预设没有可用的内置字体包。";
                _installFontsButton.Enabled = false;
                return;
            }

            _installFontsButton.Enabled = true;
            _installFontsButton.Text = "安装所需字体";
            _installFontsButton.Width = 118;
            _packageInfoLabel.Text =
                package.Name + Environment.NewLine +
                (string.IsNullOrWhiteSpace(package.Description) ? "未提供说明" : package.Description) + Environment.NewLine + Environment.NewLine +
                "适合人群：" + (string.IsNullOrWhiteSpace(package.RecommendedFor) ? "未提供说明" : package.RecommendedFor) + Environment.NewLine +
                "授权方式：" + (string.IsNullOrWhiteSpace(package.LicenseName) ? "未提供" : package.LicenseName) + Environment.NewLine +
                "字体来源：" + (string.IsNullOrWhiteSpace(package.SourceUrl) ? "未提供" : package.SourceUrl);
        }

        private async Task CheckForUpdatesAsync(bool silent)
        {
            Version currentVersion = GetCurrentVersion();
            _checkUpdatesButton.Enabled = false;
            _updateStatusLabel.Text = "正在检查 GitHub Release，请稍等...";
            _updateStatusLabel.ForeColor = UiPalette.TextSecondary;

            Log("开始检查更新。");

            try
            {
                UpdateCheckResult result = await Task.Run(delegate
                {
                    return _updateService.CheckForUpdates(currentVersion);
                });

                _latestUpdateResult = result;
                ApplyUpdateStatus(result);

                if (result != null && result.IsUpdateAvailable)
                {
                    Log("发现新版本：" + result.LatestTag);

                    if (!silent)
                    {
                        string message =
                            "发现新版本 " + FormatVersion(result.LatestVersion) +
                            (string.IsNullOrWhiteSpace(result.ReleaseName) ? string.Empty : "（" + result.ReleaseName + "）") +
                            "。要现在打开下载页吗？";

                        if (MessageBox.Show(this, message, "发现更新", MessageBoxButtons.YesNo, MessageBoxIcon.Information) == DialogResult.Yes)
                        {
                            OpenUrl(string.IsNullOrWhiteSpace(result.DownloadUrl) ? result.ReleaseUrl : result.DownloadUrl);
                        }
                    }
                }
                else if (!silent)
                {
                    MessageBox.Show(this, "当前已经是最新版本。", "检查更新", MessageBoxButtons.OK, MessageBoxIcon.Information);
                }
            }
            catch (Exception ex)
            {
                _updateStatusLabel.Text = "更新检查失败，可以稍后重试。";
                _updateStatusLabel.ForeColor = UiPalette.Warning;
                Log("检查更新失败：" + ex.Message);

                if (!silent)
                {
                    MessageBox.Show(
                        this,
                        "检查更新失败：" + ex.Message,
                        "检查更新",
                        MessageBoxButtons.OK,
                        MessageBoxIcon.Warning);
                }
            }
            finally
            {
                _checkUpdatesButton.Enabled = true;
            }
        }

        private void ApplyUpdateStatus(UpdateCheckResult result)
        {
            if (result == null)
            {
                _updateStatusLabel.Text = "暂时没有拿到版本信息。";
                _updateStatusLabel.ForeColor = UiPalette.TextSecondary;
                _openDownloadButton.Visible = false;
                return;
            }

            if (result.IsUpdateAvailable)
            {
                string published = result.PublishedAt.HasValue
                    ? "，发布于 " + result.PublishedAt.Value.ToLocalTime().ToString("yyyy-MM-dd")
                    : string.Empty;

                string summary = GetReleaseSummary(result.ReleaseBody);
                _updateStatusLabel.ForeColor = UiPalette.Accent;
                _updateStatusLabel.Text =
                    "发现新版本 v" + FormatVersion(result.LatestVersion) + published +
                    (string.IsNullOrWhiteSpace(summary) ? string.Empty : Environment.NewLine + summary);
                _openDownloadButton.Visible = true;
                _openDownloadButton.Text = "下载 " + (string.IsNullOrWhiteSpace(result.LatestTag) ? "新版本" : result.LatestTag);
                _openUpdateButton.Text = "查看更新说明";
                return;
            }

            _updateStatusLabel.ForeColor = UiPalette.Success;
            _updateStatusLabel.Text = "当前已是最新版本 v" + FormatVersion(result.CurrentVersion) + "。";
            _openDownloadButton.Visible = false;
            _openUpdateButton.Text = "打开项目主页";
        }

        private static string GetReleaseSummary(string body)
        {
            if (string.IsNullOrWhiteSpace(body))
            {
                return string.Empty;
            }

            foreach (string line in body.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries))
            {
                string trimmed = line.Trim();
                if (trimmed.Length == 0)
                {
                    continue;
                }

                if (trimmed.StartsWith("#", StringComparison.Ordinal) || trimmed.StartsWith("-", StringComparison.Ordinal))
                {
                    trimmed = trimmed.TrimStart('#', '-', ' ', '\t');
                }

                if (trimmed.Length > 58)
                {
                    trimmed = trimmed.Substring(0, 58) + "...";
                }

                return trimmed;
            }

            return string.Empty;
        }

        private async void CheckUpdatesButton_Click(object sender, EventArgs e)
        {
            await CheckForUpdatesAsync(false);
        }

        private void OpenUpdatePageButton_Click(object sender, EventArgs e)
        {
            string url = _latestUpdateResult != null && !string.IsNullOrWhiteSpace(_latestUpdateResult.ReleaseUrl)
                ? _latestUpdateResult.ReleaseUrl
                : UpdateService.RepositoryPage;

            OpenUrl(url);
        }

        private void OpenDownloadButton_Click(object sender, EventArgs e)
        {
            string url = _latestUpdateResult != null && !string.IsNullOrWhiteSpace(_latestUpdateResult.DownloadUrl)
                ? _latestUpdateResult.DownloadUrl
                : (_latestUpdateResult == null ? UpdateService.LatestReleasePage : _latestUpdateResult.ReleaseUrl);

            OpenUrl(url);
        }

        private void OpenReleasePageButton_Click(object sender, EventArgs e)
        {
            OpenUrl(UpdateService.LatestReleasePage);
        }

        private void InstallFontsButton_Click(object sender, EventArgs e)
        {
            FontPackage package = GetSelectedFontPackage();

            if (package == null)
            {
                MessageBox.Show(this, "当前预设没有可安装的字体包。", Text, MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }

            RunAction("安装预设字体", delegate
            {
                _service.InstallFontPackage(AppDomain.CurrentDomain.BaseDirectory, package);
                Log("已安装字体包：" + package.Name);
                RefreshPresetDetails();
            });
        }

        private void ApplyButton_Click(object sender, EventArgs e)
        {
            FontPreset preset = GetSelectedPreset();

            if (preset == null)
            {
                MessageBox.Show(this, "请先选择一个预设。", Text, MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }

            FontPackage package = GetSelectedFontPackage();
            IList<string> missing = _service.GetMissingFonts(preset);
            bool installFontsFirst = false;

            if (missing.Count > 0)
            {
                if (package != null)
                {
                    DialogResult choice = MessageBox.Show(
                        this,
                        "检测到以下字体未安装：" + Environment.NewLine + Environment.NewLine +
                        string.Join(Environment.NewLine, missing) +
                        Environment.NewLine + Environment.NewLine +
                        "是否先安装软件内置的字体包再继续？" + Environment.NewLine + Environment.NewLine +
                        "是：先安装字体并继续" + Environment.NewLine +
                        "否：直接继续" + Environment.NewLine +
                        "取消：返回",
                        "缺少字体",
                        MessageBoxButtons.YesNoCancel,
                        MessageBoxIcon.Warning);

                    if (choice == DialogResult.Cancel)
                    {
                        return;
                    }

                    installFontsFirst = choice == DialogResult.Yes;
                }
                else
                {
                    DialogResult proceed = MessageBox.Show(
                        this,
                        "检测到以下字体未安装：" + Environment.NewLine + Environment.NewLine +
                        string.Join(Environment.NewLine, missing) +
                        Environment.NewLine + Environment.NewLine +
                        "仍然继续吗？",
                        "缺少字体",
                        MessageBoxButtons.YesNo,
                        MessageBoxIcon.Warning);

                    if (proceed != DialogResult.Yes)
                    {
                        return;
                    }
                }
            }

            RunAction("应用预设", delegate
            {
                if (installFontsFirst && package != null)
                {
                    _service.InstallFontPackage(AppDomain.CurrentDomain.BaseDirectory, package);
                    Log("已安装字体包：" + package.Name);
                }

                string backup = _service.ApplyPreset(
                    preset,
                    _rebuildCacheCheckBox.Checked,
                    _restartExplorerCheckBox.Checked);

                Log("已应用预设：" + preset.Name);
                Log("已创建备份：" + backup);
                RefreshPresetDetails();
            });
        }

        private void BackupButton_Click(object sender, EventArgs e)
        {
            RunAction("创建备份", delegate
            {
                string backup = _service.CreateBackup();
                Log("已创建备份：" + backup);
            });
        }

        private void RestoreButton_Click(object sender, EventArgs e)
        {
            DialogResult confirm = MessageBox.Show(
                this,
                "要恢复最近一次备份并刷新资源管理器吗？",
                "恢复最近备份",
                MessageBoxButtons.YesNo,
                MessageBoxIcon.Question);

            if (confirm != DialogResult.Yes)
            {
                return;
            }

            RunAction("恢复最近备份", delegate
            {
                string restored = _service.RestoreLatestBackup(
                    _rebuildCacheCheckBox.Checked,
                    _restartExplorerCheckBox.Checked);

                Log("已恢复备份：" + restored);
                RefreshPresetDetails();
            });
        }

        private void OpenBackupsButton_Click(object sender, EventArgs e)
        {
            _service.OpenDirectory(_service.BackupRoot);
        }

        private void OpenPresetsButton_Click(object sender, EventArgs e)
        {
            _service.OpenDirectory(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Presets"));
        }

        private void OpenFontPackagesButton_Click(object sender, EventArgs e)
        {
            _service.OpenDirectory(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "FontPackages"));
        }

        private void ClearLogButton_Click(object sender, EventArgs e)
        {
            _logTextBox.Clear();
        }

        private void RunAction(string actionName, Action action)
        {
            Cursor previousCursor = Cursor.Current;

            try
            {
                Cursor.Current = Cursors.WaitCursor;
                UseWaitCursor = true;
                Enabled = false;
                Application.DoEvents();
                action();
                Log(actionName + "完成。");
            }
            catch (Exception ex)
            {
                Log(actionName + "失败：" + ex.Message);
                MessageBox.Show(this, ex.Message, actionName, MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
            finally
            {
                Enabled = true;
                UseWaitCursor = false;
                Cursor.Current = previousCursor;
            }
        }

        private static void OpenUrl(string url)
        {
            if (string.IsNullOrWhiteSpace(url))
            {
                return;
            }

            Process.Start(new ProcessStartInfo
            {
                FileName = url,
                UseShellExecute = true
            });
        }

        private static Version GetCurrentVersion()
        {
            Version version = Assembly.GetExecutingAssembly().GetName().Version;
            return version ?? new Version(0, 0, 0, 0);
        }

        private static string FormatVersion(Version version)
        {
            if (version == null)
            {
                return "0.0.0";
            }

            if (version.Revision > 0)
            {
                return version.ToString(4);
            }

            if (version.Build > 0)
            {
                return version.ToString(3);
            }

            return version.ToString(2);
        }

        private void Log(string message)
        {
            if (_logTextBox == null)
            {
                return;
            }

            string line = DateTime.Now.ToString("HH:mm:ss") + "  " + message;
            _logTextBox.AppendText(line + Environment.NewLine);
        }

        private void TryApplyWindowEffects()
        {
            try
            {
                int cornerPreference = NativeMethods.DWMWCP_ROUND;
                NativeMethods.DwmSetWindowAttribute(
                    Handle,
                    NativeMethods.DWMWA_WINDOW_CORNER_PREFERENCE,
                    ref cornerPreference,
                    Marshal.SizeOf(typeof(int)));
            }
            catch
            {
            }

            try
            {
                int backdrop = NativeMethods.DWMSBT_TRANSIENTWINDOW;
                NativeMethods.DwmSetWindowAttribute(
                    Handle,
                    NativeMethods.DWMWA_SYSTEMBACKDROP_TYPE,
                    ref backdrop,
                    Marshal.SizeOf(typeof(int)));
            }
            catch
            {
            }
        }

        private static class NativeMethods
        {
            public const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
            public const int DWMWA_SYSTEMBACKDROP_TYPE = 38;
            public const int DWMWCP_ROUND = 2;
            public const int DWMSBT_TRANSIENTWINDOW = 3;

            [DllImport("dwmapi.dll")]
            public static extern int DwmSetWindowAttribute(IntPtr hwnd, int dwAttribute, ref int pvAttribute, int cbAttribute);
        }
    }
}
