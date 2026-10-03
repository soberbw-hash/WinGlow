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
    public sealed class AppShellForm : Form
    {
        private readonly FontTunerService _service;
        private readonly UpdateService _updateService;
        private readonly List<FontPreset> _presets;
        private readonly Dictionary<string, FontPackage> _fontPackages;
        private readonly Dictionary<AppPage, Control> _pages;
        private readonly Dictionary<AppPage, ModernNavButton> _navButtons;

        private AppPage _currentPage;
        private UpdateCheckResult _latestUpdateResult;

        private Label _pageTitleLabel;
        private Label _pageSubtitleLabel;
        private Label _topVersionLabel;
        private Label _sidebarStatusLabel;

        private Label _dashboardHeroSupportLabel;
        private Label _dashboardUpdateStatusLabel;
        private Label _dashboardPresetValueLabel;
        private Label _dashboardMissingValueLabel;
        private Label _dashboardAdminValueLabel;
        private Label _dashboardUpdateValueLabel;
        private Label _dashboardPreviewTitleLabel;
        private Label _dashboardPreviewBodyLabel;
        private Label _dashboardPreviewMetaLabel;
        private Label _dashboardPackageNoteLabel;
        private Label _dashboardVersionBadgeLabel;

        private ComboBox _presetComboBox;
        private Label _descriptionLabel;
        private Label _fontHintLabel;
        private Label _packageInfoLabel;
        private Label _replacementAdminLabel;
        private FlowLayoutPanel _mappingFlow;
        private ModernButton _installFontsButton;
        private CheckBox _rebuildCacheCheckBox;
        private CheckBox _restartExplorerCheckBox;
        private CheckBox _settingsRebuildCacheCheckBox;
        private CheckBox _settingsRestartExplorerCheckBox;

        private Label _renderModeValueLabel;
        private Label _renderGammaValueLabel;
        private Label _renderContrastValueLabel;
        private Label _renderWeightValueLabel;
        private Label _renderFaceValueLabel;
        private Label _renderCurrentBodyLabel;
        private Label _renderTunedBodyLabel;
        private Label _renderPreviewMetaLabel;

        private Label _settingsAdminLabel;
        private Label _settingsUpdateLabel;
        private Label _settingsVersionLabel;
        private Label _settingsBackupHintLabel;
        private FlowLayoutPanel _backupListPanel;

        private TextBox _logTextBox;
        private ModernButton _checkUpdatesButton;
        private ModernButton _settingsCheckUpdatesButton;
        private ModernButton _openUpdateButton;
        private ModernButton _openDownloadButton;
        private ModernButton _clearLogButton;
        private bool _isLiveResizing;

        public AppShellForm()
        {
            _service = new FontTunerService();
            _updateService = new UpdateService();
            _presets = new List<FontPreset>();
            _fontPackages = new Dictionary<string, FontPackage>(StringComparer.OrdinalIgnoreCase);
            _pages = new Dictionary<AppPage, Control>();
            _navButtons = new Dictionary<AppPage, ModernNavButton>();

            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint, true);
            DoubleBuffered = true;
            AutoScaleMode = AutoScaleMode.Dpi;
            UiTypography.Initialize(AppDomain.CurrentDomain.BaseDirectory);
            Text = "WinGlow";
            Width = 1420;
            Height = 920;
            MinimumSize = new Size(1100, 760);
            StartPosition = FormStartPosition.CenterScreen;
            BackColor = UiPalette.WindowBackground;
            Font = UiTypography.Create(9.2f, FontStyle.Regular);
            Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath);

            Controls.Add(BuildShell());

            Load += AppShellForm_Load;
            Shown += AppShellForm_Shown;
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            TryApplyWindowEffects();
        }

        protected override void OnPaintBackground(PaintEventArgs e)
        {
            Rectangle bounds = ClientRectangle;

            if (_isLiveResizing)
            {
                using (SolidBrush brush = new SolidBrush(UiPalette.WindowBackground))
                {
                    e.Graphics.FillRectangle(brush, bounds);
                }

                return;
            }

            using (LinearGradientBrush brush = new LinearGradientBrush(
                bounds,
                Color.FromArgb(244, 247, 252),
                Color.FromArgb(236, 242, 250),
                90f))
            {
                e.Graphics.FillRectangle(brush, bounds);
            }

            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;

            using (SolidBrush coolBrush = new SolidBrush(Color.FromArgb(20, UiPalette.Accent)))
            using (SolidBrush warmBrush = new SolidBrush(Color.FromArgb(14, 81, 136, 214)))
            {
                e.Graphics.FillEllipse(coolBrush, new Rectangle(Width - 360, -110, 380, 380));
                e.Graphics.FillEllipse(warmBrush, new Rectangle(-150, Height - 310, 300, 300));
            }
        }

        protected override void WndProc(ref Message m)
        {
            const int WM_ENTERSIZEMOVE = 0x0231;
            const int WM_EXITSIZEMOVE = 0x0232;

            if (m.Msg == WM_ENTERSIZEMOVE)
            {
                _isLiveResizing = true;
                Invalidate();
            }
            else if (m.Msg == WM_EXITSIZEMOVE)
            {
                _isLiveResizing = false;
                Invalidate(true);
            }

            base.WndProc(ref m);
        }

        private Control BuildShell()
        {
            TableLayoutPanel shell = new TableLayoutPanel();
            shell.Dock = DockStyle.Fill;
            shell.BackColor = Color.Transparent;
            shell.ColumnCount = 2;
            shell.RowCount = 1;
            shell.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 230f));
            shell.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f));

            shell.Controls.Add(BuildSidebar(), 0, 0);
            shell.Controls.Add(BuildMainSurface(), 1, 0);

            return shell;
        }

        private Control BuildSidebar()
        {
            Panel sidebar = new Panel();
            sidebar.Dock = DockStyle.Fill;
            sidebar.BackColor = UiPalette.SidebarBackground;
            sidebar.Padding = new Padding(18, 18, 18, 18);
            sidebar.Paint += delegate(object sender, PaintEventArgs e)
            {
                using (Pen pen = new Pen(UiPalette.SidebarBorder))
                {
                    e.Graphics.DrawLine(pen, sidebar.Width - 1, 0, sidebar.Width - 1, sidebar.Height);
                }
            };

            TableLayoutPanel layout = new TableLayoutPanel();
            layout.Dock = DockStyle.Fill;
            layout.BackColor = Color.Transparent;
            layout.ColumnCount = 1;
            layout.RowCount = 3;
            layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 132f));
            layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
            layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 164f));
            sidebar.Controls.Add(layout);

            layout.Controls.Add(BuildSidebarBrand(), 0, 0);
            layout.Controls.Add(BuildSidebarNavigation(), 0, 1);
            layout.Controls.Add(BuildSidebarFooter(), 0, 2);

            return sidebar;
        }

        private Control BuildSidebarBrand()
        {
            Panel host = new Panel();
            host.Dock = DockStyle.Fill;
            host.BackColor = Color.Transparent;

            Panel mark = new Panel();
            mark.Size = new Size(54, 54);
            mark.Location = new Point(0, 8);
            mark.Paint += delegate(object sender, PaintEventArgs e)
            {
                using (System.IO.Stream source = typeof(AppShellForm).Assembly.GetManifestResourceStream("WinGlow.IconMaster.png"))
                using (Image image = Image.FromStream(source))
                {
                    e.Graphics.DrawImage(image, new Rectangle(0, 0, mark.Width, mark.Height));
                }
            };
            host.Controls.Add(mark);

            Label title = new Label();
            title.AutoSize = false;
            title.BackColor = Color.Transparent;
            title.Font = new Font(Font.FontFamily, 11.4f, FontStyle.Bold);
            title.ForeColor = UiPalette.TextPrimary;
            title.Location = new Point(0, 70);
            title.Size = new Size(176, 64);
            title.UseCompatibleTextRendering = true;
            title.Text = "WinGlow";
            host.Controls.Add(title);

            Label sub = new Label();
            sub.AutoSize = false;
            sub.BackColor = Color.Transparent;
            sub.Font = new Font(Font.FontFamily, 8.5f, FontStyle.Regular);
            sub.ForeColor = UiPalette.TextMuted;
            sub.Location = new Point(0, 118);
            sub.Size = new Size(176, 34);
            sub.Text = "替换 / 微调 / 备份 / 更新";
            host.Controls.Add(sub);

            return host;
        }

        private Control BuildSidebarNavigation()
        {
            FlowLayoutPanel nav = new FlowLayoutPanel();
            nav.Dock = DockStyle.Fill;
            nav.FlowDirection = FlowDirection.TopDown;
            nav.WrapContents = false;
            nav.BackColor = Color.Transparent;
            nav.Margin = new Padding(0);
            nav.Padding = new Padding(0, 8, 0, 0);

            nav.Controls.Add(CreateNavButton(AppPage.Dashboard, "仪表盘"));
            nav.Controls.Add(CreateNavButton(AppPage.Replacement, "字体替换"));
            nav.Controls.Add(CreateNavButton(AppPage.Rendering, "字体微调"));
            nav.Controls.Add(CreateNavButton(AppPage.Settings, "设置与备份"));

            return nav;
        }

        private Control BuildSidebarFooter()
        {
            ModernCardPanel card = CreateCard(false, 22, UiPalette.CardBackground);
            card.Padding = new Padding(16);
            card.Dock = DockStyle.Fill;

            Label label = CreateSectionEyebrow("系统状态", UiPalette.AccentTextSoft);
            label.Location = new Point(16, 14);
            card.Controls.Add(label);

            _sidebarStatusLabel = new Label();
            _sidebarStatusLabel.AutoSize = false;
            _sidebarStatusLabel.BackColor = Color.Transparent;
            _sidebarStatusLabel.Location = new Point(16, 40);
            _sidebarStatusLabel.Size = new Size(170, 36);
            _sidebarStatusLabel.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            _sidebarStatusLabel.ForeColor = UiPalette.TextPrimary;
            _sidebarStatusLabel.Text = "正在读取系统状态…";
            card.Controls.Add(_sidebarStatusLabel);

            Label hint = CreateMutedLabel("内置 3 套字体，支持备份、恢复和更新检查。", 16, 78, 170, 48);
            hint.Font = new Font(Font.FontFamily, 8.6f, FontStyle.Regular);
            hint.ForeColor = UiPalette.TextMuted;
            card.Controls.Add(hint);

            return card;
        }

        private Control BuildMainSurface()
        {
            TableLayoutPanel main = new TableLayoutPanel();
            main.Dock = DockStyle.Fill;
            main.BackColor = Color.Transparent;
            main.ColumnCount = 1;
            main.RowCount = 2;
            main.RowStyles.Add(new RowStyle(SizeType.Absolute, 72f));
            main.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));

            main.Controls.Add(BuildTopBar(), 0, 0);

            Panel contentHost = new Panel();
            contentHost.Dock = DockStyle.Fill;
            contentHost.BackColor = Color.Transparent;
            main.Controls.Add(contentHost, 0, 1);

            BuildPages(contentHost);

            return main;
        }

        private Control BuildTopBar()
        {
            Panel bar = new Panel();
            bar.Dock = DockStyle.Fill;
            bar.BackColor = Color.FromArgb(248, 250, 253);
            bar.Padding = new Padding(24, 14, 24, 12);
            bar.Paint += delegate(object sender, PaintEventArgs e)
            {
                using (Pen pen = new Pen(Color.FromArgb(210, 221, 231)))
                {
                    e.Graphics.DrawLine(pen, 0, bar.Height - 1, bar.Width, bar.Height - 1);
                }
            };

            Panel left = new Panel();
            left.Dock = DockStyle.Fill;
            left.BackColor = Color.Transparent;
            bar.Controls.Add(left);

            _pageTitleLabel = new Label();
            _pageTitleLabel.AutoSize = true;
            _pageTitleLabel.BackColor = Color.Transparent;
            _pageTitleLabel.Font = new Font(Font.FontFamily, 14f, FontStyle.Bold);
            _pageTitleLabel.ForeColor = UiPalette.TextPrimary;
            _pageTitleLabel.Location = new Point(0, 0);
            left.Controls.Add(_pageTitleLabel);

            _pageSubtitleLabel = new Label();
            _pageSubtitleLabel.AutoSize = true;
            _pageSubtitleLabel.BackColor = Color.Transparent;
            _pageSubtitleLabel.Font = new Font(Font.FontFamily, 8.8f, FontStyle.Regular);
            _pageSubtitleLabel.ForeColor = UiPalette.TextMuted;
            _pageSubtitleLabel.Location = new Point(0, 28);
            _pageSubtitleLabel.Visible = false;
            left.Controls.Add(_pageSubtitleLabel);

            FlowLayoutPanel right = new FlowLayoutPanel();
            right.Dock = DockStyle.Right;
            right.AutoSize = true;
            right.BackColor = Color.Transparent;
            right.WrapContents = false;
            bar.Controls.Add(right);

            _topVersionLabel = CreateBadgeLabel("v" + FormatVersion(GetCurrentVersion()), UiPalette.AccentSoft, UiPalette.AccentTextSoft);
            _topVersionLabel.Margin = new Padding(0, 4, 10, 0);
            right.Controls.Add(_topVersionLabel);

            ModernButton openRepoButton = BuildButton("GitHub", OpenRepositoryButton_Click, ModernButtonStyle.Ghost, 82);
            openRepoButton.Margin = new Padding(0, 0, 0, 0);
            right.Controls.Add(openRepoButton);

            return bar;
        }

        private void BuildPages(Control contentHost)
        {
            RegisterPage(AppPage.Dashboard, BuildDashboardPage(), contentHost);
            RegisterPage(AppPage.Replacement, BuildReplacementPage(), contentHost);
            RegisterPage(AppPage.Rendering, BuildRenderingPage(), contentHost);
            RegisterPage(AppPage.Settings, BuildSettingsPage(), contentHost);
            ShowPage(AppPage.Dashboard);
        }

        private void RegisterPage(AppPage page, Control control, Control host)
        {
            control.Visible = false;
            control.Dock = DockStyle.Fill;
            host.Controls.Add(control);
            _pages[page] = control;
        }

        private ModernNavButton CreateNavButton(AppPage page, string text)
        {
            ModernNavButton button = new ModernNavButton();
            button.Text = text;
            button.Width = 188;
            button.Margin = new Padding(0, 0, 0, 8);
            button.Click += delegate { ShowPage(page); };
            _navButtons[page] = button;
            return button;
        }

        private void ShowPage(AppPage page)
        {
            _currentPage = page;

            foreach (KeyValuePair<AppPage, Control> pair in _pages)
            {
                pair.Value.Visible = pair.Key == page;
            }

            foreach (KeyValuePair<AppPage, ModernNavButton> pair in _navButtons)
            {
                pair.Value.Selected = pair.Key == page;
            }

            if (_pageTitleLabel != null)
            {
                _pageTitleLabel.Text = GetPageTitle(page);
            }

            if (_pageSubtitleLabel != null)
            {
                _pageSubtitleLabel.Text = GetPageSubtitle(page);
            }
        }

        private static string GetPageTitle(AppPage page)
        {
            if (page == AppPage.Replacement)
            {
                return "字体替换";
            }

            if (page == AppPage.Rendering)
            {
                return "字体微调";
            }

            if (page == AppPage.Settings)
            {
                return "设置与备份";
            }

            return "仪表盘";
        }

        private static string GetPageSubtitle(AppPage page)
        {
            if (page == AppPage.Replacement)
            {
                return "选择预设、安装字体、应用系统映射";
            }

            if (page == AppPage.Rendering)
            {
                return "查看当前预设的渲染参数和样张";
            }

            if (page == AppPage.Settings)
            {
                return "管理快照、更新检查和资源目录";
            }

            return "用更舒服的方式整理 Windows 字体体验";
        }

        private Panel CreatePage(out TableLayoutPanel canvas)
        {
            Panel page = new Panel();
            page.Dock = DockStyle.Fill;
            page.AutoScroll = true;
            page.BackColor = Color.Transparent;
            page.Padding = new Padding(24, 24, 24, 24);
            EnableDoubleBuffer(page);

            canvas = new TableLayoutPanel();
            canvas.Dock = DockStyle.Top;
            canvas.AutoSize = true;
            canvas.AutoSizeMode = AutoSizeMode.GrowAndShrink;
            canvas.BackColor = Color.Transparent;
            canvas.ColumnCount = 1;
            canvas.RowCount = 0;
            canvas.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f));
            EnableDoubleBuffer(canvas);
            page.Controls.Add(canvas);

            TableLayoutPanel layoutCanvas = canvas;

            EventHandler resizeHandler = delegate
            {
                int preferred = page.ClientSize.Width - page.Padding.Horizontal - 4;
                layoutCanvas.Width = Math.Max(760, preferred);
            };

            page.Resize += resizeHandler;
            resizeHandler(page, EventArgs.Empty);
            return page;
        }

        private static TableLayoutPanel CreateSplitRow(int columnCount, int height)
        {
            TableLayoutPanel row = new TableLayoutPanel();
            row.Dock = DockStyle.Top;
            row.Height = height;
            row.BackColor = Color.Transparent;
            row.ColumnCount = columnCount;
            row.RowCount = 1;

            for (int index = 0; index < columnCount; index++)
            {
                row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f / columnCount));
            }

            row.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
            return row;
        }

        private static void EnableDoubleBuffer(Control control)
        {
            typeof(Control).GetProperty("DoubleBuffered", BindingFlags.Instance | BindingFlags.NonPublic)
                .SetValue(control, true, null);
        }

        private static ModernCardPanel CreateCard(bool useGradient, int radius, Color fillColor)
        {
            ModernCardPanel card = new ModernCardPanel();
            card.UseGradient = useGradient;
            card.CornerRadius = radius;
            card.FillColor = fillColor;
            card.BorderColor = UiPalette.Border;
            return card;
        }

        private static Label CreateSectionTitle(string text)
        {
            Label label = new Label();
            label.AutoSize = true;
            label.BackColor = Color.Transparent;
            label.Font = UiTypography.Create(12.4f, FontStyle.Bold);
            label.ForeColor = UiPalette.TextPrimary;
            label.Text = text;
            return label;
        }

        private static Label CreateSectionEyebrow(string text, Color foreColor)
        {
            Label label = new Label();
            label.AutoSize = true;
            label.BackColor = Color.Transparent;
            label.Font = UiTypography.Create(8.5f, FontStyle.Bold);
            label.ForeColor = foreColor;
            label.Text = text;
            return label;
        }

        private static Label CreateMutedLabel(string text, int x, int y, int width, int height)
        {
            Label label = new Label();
            label.AutoSize = false;
            label.BackColor = Color.Transparent;
            label.Location = new Point(x, y);
            label.Size = new Size(width, height);
            label.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            label.Font = UiTypography.Create(9f, FontStyle.Regular);
            label.ForeColor = UiPalette.TextSecondary;
            label.UseCompatibleTextRendering = false;
            label.Text = text;
            return label;
        }

        private static Label CreateBadgeLabel(string text, Color backColor, Color foreColor)
        {
            Label label = new Label();
            label.AutoSize = true;
            label.BackColor = backColor;
            label.ForeColor = foreColor;
            label.Font = UiTypography.Create(8.4f, FontStyle.Bold);
            label.Padding = new Padding(10, 5, 10, 5);
            label.Text = text;
            return label;
        }

        private static ModernButton BuildButton(string text, EventHandler onClick, ModernButtonStyle style, int width)
        {
            ModernButton button = new ModernButton();
            button.Text = text;
            int measuredWidth = TextRenderer.MeasureText(text, button.Font).Width + 24;
            button.Width = width > 0 ? Math.Max(width, measuredWidth) : Math.Max(88, measuredWidth);
            button.Height = 38;
            button.ButtonStyle = style;
            button.CornerRadius = 12;
            button.Margin = new Padding(0, 0, 8, 8);
            button.Click += onClick;
            return button;
        }

        private static void BindControlWidth(Control container, Control child, int left, int right, int minWidth)
        {
            EventHandler apply = delegate
            {
                int width = Math.Max(minWidth, container.ClientSize.Width - left - right);
                if (child.Width != width)
                {
                    child.Width = width;
                }
            };

            container.Resize += apply;
            apply(container, EventArgs.Empty);
        }

        private static void BindWrappedLabel(Control container, Label label, int left, int top, int right, int minWidth, int minHeight)
        {
            EventHandler apply = delegate
            {
                int width = Math.Max(minWidth, container.ClientSize.Width - left - right);
                Size measured = TextRenderer.MeasureText(
                    label.Text ?? string.Empty,
                    label.Font,
                    new Size(width, 0),
                    TextFormatFlags.WordBreak | TextFormatFlags.NoPadding);
                int height = Math.Max(minHeight, measured.Height + 4);

                if (label.Location.X != left || label.Location.Y != top)
                {
                    label.Location = new Point(left, top);
                }

                if (label.Width != width || label.Height != height)
                {
                    label.Size = new Size(width, height);
                }
            };

            container.Resize += apply;
            label.TextChanged += apply;
            apply(container, EventArgs.Empty);
        }

        private static void BindFlowWidth(Control container, FlowLayoutPanel flow, int left, int right, int minWidth)
        {
            EventHandler apply = delegate
            {
                int width = Math.Max(minWidth, container.ClientSize.Width - left - right);
                if (flow.Width != width)
                {
                    flow.Width = width;
                }

                if (flow.MaximumSize.Width != width)
                {
                    flow.MaximumSize = new Size(width, 0);
                }
            };

            container.Resize += apply;
            apply(container, EventArgs.Empty);
        }

        private static void ConfigureResponsiveRow(TableLayoutPanel row, Control first, Control second, int breakPoint, int wideHeight, int stackedHeight, float wideFirstPercent, float wideSecondPercent)
        {
            bool? lastStacked = null;
            EventHandler apply = delegate
            {
                bool stacked = row.Width < breakPoint;
                int targetHeight = stacked ? stackedHeight : wideHeight;

                if (lastStacked.HasValue && lastStacked.Value == stacked)
                {
                    if (row.Height != targetHeight)
                    {
                        row.Height = targetHeight;
                    }

                    return;
                }

                row.SuspendLayout();
                row.Controls.Clear();
                row.ColumnStyles.Clear();
                row.RowStyles.Clear();

                if (stacked)
                {
                    row.ColumnCount = 1;
                    row.RowCount = 2;
                    row.Height = stackedHeight;
                    row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f));
                    row.RowStyles.Add(new RowStyle(SizeType.Percent, 50f));
                    row.RowStyles.Add(new RowStyle(SizeType.Percent, 50f));
                    first.Margin = new Padding(0, 0, 0, 16);
                    second.Margin = new Padding(0);
                    row.Controls.Add(first, 0, 0);
                    row.Controls.Add(second, 0, 1);
                }
                else
                {
                    row.ColumnCount = 2;
                    row.RowCount = 1;
                    row.Height = wideHeight;
                    row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, wideFirstPercent));
                    row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, wideSecondPercent));
                    row.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
                    first.Margin = new Padding(0, 0, 10, 0);
                    second.Margin = new Padding(10, 0, 0, 0);
                    row.Controls.Add(first, 0, 0);
                    row.Controls.Add(second, 1, 0);
                }

                row.ResumeLayout();
                lastStacked = stacked;
            };

            row.Resize += apply;
            apply(row, EventArgs.Empty);
        }

        private static void ConfigureResponsivePreviewSplit(TableLayoutPanel split, Control leftPane, Control rightPane, int breakPoint, int wideHeight, int stackedHeight)
        {
            bool? lastStacked = null;
            EventHandler apply = delegate
            {
                bool stacked = split.Width < breakPoint;
                int targetHeight = stacked ? stackedHeight : wideHeight;

                if (lastStacked.HasValue && lastStacked.Value == stacked)
                {
                    if (split.Height != targetHeight)
                    {
                        split.Height = targetHeight;
                    }

                    return;
                }

                split.SuspendLayout();
                split.Controls.Clear();
                split.ColumnStyles.Clear();
                split.RowStyles.Clear();

                if (stacked)
                {
                    split.ColumnCount = 1;
                    split.RowCount = 2;
                    split.Height = stackedHeight;
                    split.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f));
                    split.RowStyles.Add(new RowStyle(SizeType.Percent, 50f));
                    split.RowStyles.Add(new RowStyle(SizeType.Percent, 50f));
                    leftPane.Margin = new Padding(0, 0, 0, 12);
                    rightPane.Margin = new Padding(0);
                    split.Controls.Add(leftPane, 0, 0);
                    split.Controls.Add(rightPane, 0, 1);
                }
                else
                {
                    split.ColumnCount = 2;
                    split.RowCount = 1;
                    split.Height = wideHeight;
                    split.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50f));
                    split.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50f));
                    split.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
                    leftPane.Margin = new Padding(0, 0, 10, 0);
                    rightPane.Margin = new Padding(10, 0, 0, 0);
                    split.Controls.Add(leftPane, 0, 0);
                    split.Controls.Add(rightPane, 1, 0);
                }

                split.ResumeLayout();
                lastStacked = stacked;
            };

            split.Resize += apply;
            apply(split, EventArgs.Empty);
        }

        private Control BuildDashboardPage()
        {
            TableLayoutPanel canvas;
            Panel page = CreatePage(out canvas);

            canvas.Controls.Add(BuildDashboardHeroCard(), 0, 0);
            canvas.Controls.Add(BuildDashboardStatsRow(), 0, 1);
            canvas.Controls.Add(BuildDashboardMainRow(), 0, 2);

            return page;
        }

        private Control BuildDashboardHeroCard()
        {
            ModernCardPanel hero = CreateCard(true, 32, UiPalette.CardBackground);
            hero.ShowGlow = true;
            hero.Dock = DockStyle.Top;
            hero.Height = 214;
            hero.Margin = new Padding(0, 0, 0, 18);
            hero.Padding = new Padding(26, 22, 26, 22);

            TableLayoutPanel layout = new TableLayoutPanel();
            layout.Dock = DockStyle.Fill;
            layout.BackColor = Color.Transparent;
            layout.ColumnCount = 2;
            layout.RowCount = 1;
            layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 68f));
            layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 32f));
            hero.Controls.Add(layout);

            Panel left = new Panel();
            left.Dock = DockStyle.Fill;
            left.BackColor = Color.Transparent;
            layout.Controls.Add(left, 0, 0);

            Label title = new Label();
            title.AutoSize = false;
            title.BackColor = Color.Transparent;
            title.Location = new Point(0, 0);
            title.Size = new Size(620, 28);
            title.Font = new Font(Font.FontFamily, 12.8f, FontStyle.Bold);
            title.UseCompatibleTextRendering = false;
            title.AutoEllipsis = true;
            title.ForeColor = UiPalette.TextPrimary;
            title.Text = "把系统字体换成你真正想看的样子";
            left.Controls.Add(title);

            _dashboardHeroSupportLabel = new Label();
            _dashboardHeroSupportLabel.AutoSize = false;
            _dashboardHeroSupportLabel.BackColor = Color.Transparent;
            _dashboardHeroSupportLabel.Location = new Point(0, 36);
            _dashboardHeroSupportLabel.Size = new Size(620, 38);
            _dashboardHeroSupportLabel.Font = new Font(Font.FontFamily, 9.2f, FontStyle.Regular);
            _dashboardHeroSupportLabel.ForeColor = UiPalette.TextSecondary;
            _dashboardHeroSupportLabel.Text = "当前方案齐了字体就能直接应用，不齐的话先安装一遍。";
            _dashboardHeroSupportLabel.UseCompatibleTextRendering = false;
            left.Controls.Add(_dashboardHeroSupportLabel);
            BindControlWidth(left, title, 0, 0, 320);
            BindWrappedLabel(left, _dashboardHeroSupportLabel, 0, 36, 0, 320, 22);

            _dashboardVersionBadgeLabel = CreateBadgeLabel("当前版本", UiPalette.AccentSoft, UiPalette.AccentTextSoft);
            _dashboardVersionBadgeLabel.Location = new Point(0, 78);
            left.Controls.Add(_dashboardVersionBadgeLabel);

            FlowLayoutPanel heroActions = new FlowLayoutPanel();
            heroActions.AutoSize = true;
            heroActions.BackColor = Color.Transparent;
            heroActions.Location = new Point(0, 112);
            heroActions.MaximumSize = new Size(360, 0);
            heroActions.WrapContents = true;
            left.Controls.Add(heroActions);
            BindFlowWidth(left, heroActions, 0, 0, 240);

            ModernButton goReplacementButton = BuildButton("去字体替换", delegate { ShowPage(AppPage.Replacement); }, ModernButtonStyle.Primary, 118);
            goReplacementButton.Margin = new Padding(0, 0, 8, 8);
            heroActions.Controls.Add(goReplacementButton);

            ModernButton applyFromDashboardButton = BuildButton("立即应用当前预设", ApplyButton_Click, ModernButtonStyle.Secondary, 150);
            applyFromDashboardButton.Margin = new Padding(0, 0, 0, 8);
            heroActions.Controls.Add(applyFromDashboardButton);

            Panel right = new Panel();
            right.Dock = DockStyle.Fill;
            right.BackColor = Color.Transparent;
            layout.Controls.Add(right, 1, 0);

            Label updateTitle = CreateSectionTitle("版本与更新");
            updateTitle.Location = new Point(0, 0);
            right.Controls.Add(updateTitle);

            _dashboardUpdateStatusLabel = new Label();
            _dashboardUpdateStatusLabel.AutoSize = false;
            _dashboardUpdateStatusLabel.BackColor = Color.Transparent;
            _dashboardUpdateStatusLabel.Location = new Point(0, 32);
            _dashboardUpdateStatusLabel.Size = new Size(210, 42);
            _dashboardUpdateStatusLabel.Font = new Font(Font.FontFamily, 9.2f, FontStyle.Regular);
            _dashboardUpdateStatusLabel.ForeColor = UiPalette.TextSecondary;
            _dashboardUpdateStatusLabel.Text = "启动后会自动检查一次。";
            _dashboardUpdateStatusLabel.UseCompatibleTextRendering = false;
            right.Controls.Add(_dashboardUpdateStatusLabel);
            BindWrappedLabel(right, _dashboardUpdateStatusLabel, 0, 32, 0, 180, 24);

            FlowLayoutPanel updateActions = new FlowLayoutPanel();
            updateActions.AutoSize = true;
            updateActions.BackColor = Color.Transparent;
            updateActions.Location = new Point(0, 86);
            updateActions.MaximumSize = new Size(220, 0);
            updateActions.WrapContents = true;
            right.Controls.Add(updateActions);
            BindFlowWidth(right, updateActions, 0, 0, 180);

            _checkUpdatesButton = BuildButton("检查更新", CheckUpdatesButton_Click, ModernButtonStyle.Primary, 102);
            _checkUpdatesButton.Margin = new Padding(0, 0, 8, 8);
            updateActions.Controls.Add(_checkUpdatesButton);

            _openUpdateButton = BuildButton("打开发布页", OpenUpdatePageButton_Click, ModernButtonStyle.Secondary, 108);
            _openUpdateButton.Margin = new Padding(0, 0, 0, 8);
            updateActions.Controls.Add(_openUpdateButton);

            _openDownloadButton = BuildButton("下载新版", OpenDownloadButton_Click, ModernButtonStyle.Secondary, 118);
            _openDownloadButton.Margin = new Padding(0);
            _openDownloadButton.Visible = false;
            updateActions.Controls.Add(_openDownloadButton);

            bool? heroStacked = null;
            EventHandler applyHeroLayout = delegate
            {
                bool stacked = hero.ClientSize.Width < 1080;
                if (heroStacked.HasValue && heroStacked.Value == stacked)
                {
                    return;
                }

                layout.SuspendLayout();
                layout.Controls.Clear();
                layout.ColumnStyles.Clear();
                layout.RowStyles.Clear();

                if (stacked)
                {
                    hero.Height = 270;
                    layout.ColumnCount = 1;
                    layout.RowCount = 2;
                    layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f));
                    layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 148f));
                    layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 94f));
                    left.Margin = new Padding(0, 0, 0, 12);
                    right.Margin = new Padding(0);
                    layout.Controls.Add(left, 0, 0);
                    layout.Controls.Add(right, 0, 1);
                }
                else
                {
                    hero.Height = 214;
                    layout.ColumnCount = 2;
                    layout.RowCount = 1;
                    layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 62f));
                    layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 38f));
                    layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
                    left.Margin = new Padding(0, 0, 12, 0);
                    right.Margin = new Padding(0);
                    layout.Controls.Add(left, 0, 0);
                    layout.Controls.Add(right, 1, 0);
                }

                layout.ResumeLayout();
                heroStacked = stacked;
            };

            hero.Resize += applyHeroLayout;
            applyHeroLayout(hero, EventArgs.Empty);

            return hero;
        }

        private Control BuildDashboardStatsRow()
        {
            TableLayoutPanel row = CreateSplitRow(4, 124);
            row.Margin = new Padding(0, 0, 0, 18);

            row.Controls.Add(CreateStatCard("当前预设", "正在准备应用的字体方案", out _dashboardPresetValueLabel, UiPalette.AccentSoft, UiPalette.AccentTextSoft), 0, 0);
            row.Controls.Add(CreateStatCard("缺失字体", "建议先安装后再应用", out _dashboardMissingValueLabel, UiPalette.WarningSoft, UiPalette.Warning), 1, 0);
            row.Controls.Add(CreateStatCard("管理员权限", "写入系统设置需要管理员", out _dashboardAdminValueLabel, UiPalette.SuccessSoft, UiPalette.Success), 2, 0);
            row.Controls.Add(CreateStatCard("更新状态", "本地版本与远端版本对比", out _dashboardUpdateValueLabel, Color.FromArgb(235, 241, 251), UiPalette.TextSecondary), 3, 0);

            return row;
        }

        private Control BuildDashboardMainRow()
        {
            TableLayoutPanel row = new TableLayoutPanel();
            row.Dock = DockStyle.Top;
            row.Height = 404;
            row.Margin = new Padding(0, 0, 0, 18);
            row.BackColor = Color.Transparent;
            row.ColumnCount = 2;
            row.RowCount = 1;
            row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 58f));
            row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 42f));

            Control previewCard = BuildDashboardPreviewCard();
            Control resourceCard = BuildDashboardResourceCard();
            row.Controls.Add(previewCard, 0, 0);
            row.Controls.Add(resourceCard, 1, 0);
            ConfigureResponsiveRow(row, previewCard, resourceCard, 1240, 420, 780, 58f, 42f);

            return row;
        }

        private Control BuildDashboardPreviewCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Fill;
            card.Margin = new Padding(0, 0, 10, 0);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("当前效果预览");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            Label sub = CreateMutedLabel("会根据当前预设切换样张字体和字重。", 22, 52, 470, 20);
            card.Controls.Add(sub);

            ModernCardPanel sampleCard = CreateCard(false, 24, UiPalette.CardAltBackground);
            sampleCard.ShowShadow = false;
            sampleCard.BorderColor = UiPalette.Border;
            sampleCard.Location = new Point(22, 88);
            sampleCard.Size = new Size(470, 232);
            sampleCard.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            sampleCard.Padding = new Padding(18);
            card.Controls.Add(sampleCard);
            BindControlWidth(card, sampleCard, 22, 22, 340);

            _dashboardPreviewTitleLabel = new Label();
            _dashboardPreviewTitleLabel.AutoSize = false;
            _dashboardPreviewTitleLabel.BackColor = Color.Transparent;
            _dashboardPreviewTitleLabel.Location = new Point(18, 18);
            _dashboardPreviewTitleLabel.Size = new Size(390, 42);
            _dashboardPreviewTitleLabel.Font = new Font(Font.FontFamily, 16.6f, FontStyle.Bold);
            _dashboardPreviewTitleLabel.ForeColor = UiPalette.TextPrimary;
            _dashboardPreviewTitleLabel.Text = "敏捷的棕色狐狸 0123456789";
            _dashboardPreviewTitleLabel.UseCompatibleTextRendering = false;
            sampleCard.Controls.Add(_dashboardPreviewTitleLabel);

            _dashboardPreviewBodyLabel = new Label();
            _dashboardPreviewBodyLabel.AutoSize = false;
            _dashboardPreviewBodyLabel.BackColor = Color.Transparent;
            _dashboardPreviewBodyLabel.Location = new Point(18, 70);
            _dashboardPreviewBodyLabel.Size = new Size(390, 92);
            _dashboardPreviewBodyLabel.Font = new Font(Font.FontFamily, 10f, FontStyle.Regular);
            _dashboardPreviewBodyLabel.ForeColor = UiPalette.TextSecondary;
            _dashboardPreviewBodyLabel.Text = "Aa The quick brown fox jumps over the lazy dog.\r\n资源管理器、桌面图标和常见正文区域会尽量统一风格。";
            _dashboardPreviewBodyLabel.UseCompatibleTextRendering = false;
            sampleCard.Controls.Add(_dashboardPreviewBodyLabel);

            _dashboardPreviewMetaLabel = CreateMutedLabel("", 18, 176, 390, 28);
            _dashboardPreviewMetaLabel.Font = new Font(Font.FontFamily, 8.8f, FontStyle.Bold);
            _dashboardPreviewMetaLabel.ForeColor = UiPalette.TextMuted;
            sampleCard.Controls.Add(_dashboardPreviewMetaLabel);

            Label tip = CreateMutedLabel("想继续微调的话，直接去“字体微调”页看参数。", 22, 330, 470, 18);
            card.Controls.Add(tip);
            BindControlWidth(card, sub, 22, 22, 300);
            BindControlWidth(sampleCard, _dashboardPreviewTitleLabel, 18, 18, 260);
            BindControlWidth(sampleCard, _dashboardPreviewBodyLabel, 18, 18, 260);
            BindControlWidth(sampleCard, _dashboardPreviewMetaLabel, 18, 18, 260);
            BindControlWidth(card, tip, 22, 22, 280);

            return card;
        }

        private Control BuildDashboardResourceCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Fill;
            card.Margin = new Padding(10, 0, 0, 0);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("当前资源");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            _dashboardPackageNoteLabel = new Label();
            _dashboardPackageNoteLabel.AutoSize = false;
            _dashboardPackageNoteLabel.BackColor = Color.Transparent;
            _dashboardPackageNoteLabel.Location = new Point(22, 54);
            _dashboardPackageNoteLabel.Size = new Size(300, 64);
            _dashboardPackageNoteLabel.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _dashboardPackageNoteLabel.Font = new Font(Font.FontFamily, 10f, FontStyle.Regular);
            _dashboardPackageNoteLabel.ForeColor = UiPalette.TextSecondary;
            _dashboardPackageNoteLabel.Text = "当前预设对应的字体包信息会显示在这里。";
            _dashboardPackageNoteLabel.UseCompatibleTextRendering = false;
            card.Controls.Add(_dashboardPackageNoteLabel);

            FlowLayoutPanel actions = new FlowLayoutPanel();
            actions.AutoSize = true;
            actions.BackColor = Color.Transparent;
            actions.Location = new Point(22, 126);
            actions.MaximumSize = new Size(280, 0);
            actions.WrapContents = true;
            card.Controls.Add(actions);
            BindFlowWidth(card, actions, 22, 22, 220);

            ModernButton installButton = BuildButton("安装当前字体", InstallFontsButton_Click, ModernButtonStyle.Primary, 0);
            installButton.Margin = new Padding(0, 0, 8, 8);
            actions.Controls.Add(installButton);

            ModernButton backupButton = BuildButton("创建备份", BackupButton_Click, ModernButtonStyle.Secondary, 0);
            backupButton.Margin = new Padding(0);
            actions.Controls.Add(backupButton);

            ModernCardPanel tips = CreateCard(false, 24, Color.FromArgb(246, 249, 253));
            tips.ShowShadow = false;
            tips.Location = new Point(22, 198);
            tips.Size = new Size(300, 110);
            tips.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            tips.Padding = new Padding(16);
            card.Controls.Add(tips);

            Label tipsTitle = CreateSectionEyebrow("使用顺序", UiPalette.AccentTextSoft);
            tipsTitle.Location = new Point(16, 12);
            tips.Controls.Add(tipsTitle);

            Label tipsBody = CreateMutedLabel("先选预设，再装字体，最后一键应用。", 16, 36, 268, 46);
            tips.Controls.Add(tipsBody);
            BindWrappedLabel(card, _dashboardPackageNoteLabel, 22, 54, 22, 240, 52);
            BindFlowWidth(card, actions, 22, 22, 240);
            BindControlWidth(card, tips, 22, 22, 240);
            BindWrappedLabel(tips, tipsBody, 16, 36, 16, 208, 36);

            return card;
        }

        private Control BuildReplacementPage()
        {
            TableLayoutPanel canvas;
            Panel page = CreatePage(out canvas);

            canvas.Controls.Add(BuildPageIntro("字体替换", "选一套预设，装好字体后再应用到系统。"), 0, 0);

            TableLayoutPanel topRow = new TableLayoutPanel();
            topRow.Dock = DockStyle.Top;
            topRow.Height = 596;
            topRow.Margin = new Padding(0, 0, 0, 18);
            topRow.BackColor = Color.Transparent;
            topRow.ColumnCount = 2;
            topRow.RowCount = 1;
            topRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 58f));
            topRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 42f));
            Control presetCard = BuildPresetCard();
            Control statusCard = BuildStatusCard();
            topRow.Controls.Add(presetCard, 0, 0);
            topRow.Controls.Add(statusCard, 1, 0);
            ConfigureResponsiveRow(topRow, presetCard, statusCard, 1280, 612, 1000, 58f, 42f);
            canvas.Controls.Add(topRow, 0, 1);

            canvas.Controls.Add(BuildMappingCard(), 0, 2);

            return page;
        }

        private Control BuildPresetCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Fill;
            card.Margin = new Padding(0, 0, 10, 0);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("预设与操作");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            Label sub = CreateMutedLabel("建议顺序：先安装字体，再应用预设。", 22, 54, 560, 20);
            card.Controls.Add(sub);

            Label comboLabel = new Label();
            comboLabel.AutoSize = true;
            comboLabel.BackColor = Color.Transparent;
            comboLabel.Location = new Point(22, 102);
            comboLabel.Font = new Font(Font.FontFamily, 9.4f, FontStyle.Regular);
            comboLabel.ForeColor = UiPalette.TextSecondary;
            comboLabel.Text = "预设";
            card.Controls.Add(comboLabel);

            _presetComboBox = new ComboBox();
            _presetComboBox.DropDownStyle = ComboBoxStyle.DropDownList;
            _presetComboBox.FlatStyle = FlatStyle.Flat;
            _presetComboBox.Font = new Font(Font.FontFamily, 10.4f, FontStyle.Bold);
            _presetComboBox.Location = new Point(68, 96);
            _presetComboBox.Size = new Size(250, 34);
            _presetComboBox.SelectedIndexChanged += PresetComboBox_SelectedIndexChanged;
            card.Controls.Add(_presetComboBox);

            ModernCardPanel hintCard = CreateCard(false, 22, Color.FromArgb(248, 250, 253));
            hintCard.ShowShadow = false;
            hintCard.Location = new Point(22, 150);
            hintCard.Size = new Size(448, 98);
            hintCard.Padding = new Padding(16);
            card.Controls.Add(hintCard);
            BindControlWidth(card, hintCard, 22, 22, 320);

            Label hintTitle = CreateSectionEyebrow("字体状态", UiPalette.AccentTextSoft);
            hintTitle.Location = new Point(16, 12);
            hintCard.Controls.Add(hintTitle);

            _fontHintLabel = CreateMutedLabel("正在读取当前预设需要的字体…", 16, 34, 528, 36);
            _fontHintLabel.Size = new Size(416, 48);
            hintCard.Controls.Add(_fontHintLabel);
            BindWrappedLabel(hintCard, _fontHintLabel, 16, 34, 16, 260, 40);

            FlowLayoutPanel actions = new FlowLayoutPanel();
            actions.AutoSize = true;
            actions.BackColor = Color.Transparent;
            actions.Location = new Point(22, 248);
            actions.MaximumSize = new Size(448, 0);
            actions.WrapContents = true;
            card.Controls.Add(actions);
            BindFlowWidth(card, actions, 22, 22, 320);

            actions.Controls.Add(BuildButton("应用预设", ApplyButton_Click, ModernButtonStyle.Primary, 0));
            _installFontsButton = BuildButton("安装字体", InstallFontsButton_Click, ModernButtonStyle.Secondary, 0);
            actions.Controls.Add(_installFontsButton);
            actions.Controls.Add(BuildButton("创建备份", BackupButton_Click, ModernButtonStyle.Secondary, 0));
            actions.Controls.Add(BuildButton("恢复最近", RestoreButton_Click, ModernButtonStyle.Secondary, 0));
            actions.Controls.Add(BuildButton("恢复默认", ResetWindowsDefaultsButton_Click, ModernButtonStyle.Ghost, 0));

            ModernCardPanel descriptionCard = CreateCard(false, 24, UiPalette.CardAltBackground);
            descriptionCard.ShowShadow = false;
            descriptionCard.Location = new Point(22, 390);
            descriptionCard.Size = new Size(448, 132);
            descriptionCard.Padding = new Padding(16);
            card.Controls.Add(descriptionCard);
            BindControlWidth(card, descriptionCard, 22, 22, 320);

            Label descriptionTitle = CreateSectionEyebrow("当前预设说明", UiPalette.TextSecondary);
            descriptionTitle.Location = new Point(16, 12);
            descriptionCard.Controls.Add(descriptionTitle);

            _descriptionLabel = CreateMutedLabel("正在加载说明…", 16, 36, 528, 64);
            _descriptionLabel.Size = new Size(416, 78);
            _descriptionLabel.Font = new Font(Font.FontFamily, 10f, FontStyle.Regular);
            descriptionCard.Controls.Add(_descriptionLabel);
            BindWrappedLabel(descriptionCard, _descriptionLabel, 16, 36, 16, 260, 56);

            return card;
        }

        private Control BuildStatusCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Fill;
            card.Margin = new Padding(10, 0, 0, 0);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("状态与资源");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            _replacementAdminLabel = CreateMutedLabel("正在读取权限状态…", 22, 56, 388, 42);
            _replacementAdminLabel.Size = new Size(308, 56);
            _replacementAdminLabel.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            card.Controls.Add(_replacementAdminLabel);
            BindWrappedLabel(card, _replacementAdminLabel, 22, 56, 22, 240, 40);

            _rebuildCacheCheckBox = new CheckBox();
            _rebuildCacheCheckBox.AutoSize = true;
            _rebuildCacheCheckBox.BackColor = Color.Transparent;
            _rebuildCacheCheckBox.Checked = true;
            _rebuildCacheCheckBox.ForeColor = UiPalette.TextSecondary;
            _rebuildCacheCheckBox.Location = new Point(22, 112);
            _rebuildCacheCheckBox.Text = "应用后重建字体缓存";
            _rebuildCacheCheckBox.CheckedChanged += ReplacementOptionChanged;
            card.Controls.Add(_rebuildCacheCheckBox);

            _restartExplorerCheckBox = new CheckBox();
            _restartExplorerCheckBox.AutoSize = true;
            _restartExplorerCheckBox.BackColor = Color.Transparent;
            _restartExplorerCheckBox.Checked = true;
            _restartExplorerCheckBox.ForeColor = UiPalette.TextSecondary;
            _restartExplorerCheckBox.Location = new Point(22, 140);
            _restartExplorerCheckBox.Text = "应用后重启资源管理器";
            _restartExplorerCheckBox.CheckedChanged += ReplacementOptionChanged;
            card.Controls.Add(_restartExplorerCheckBox);

            ModernCardPanel packageCard = CreateCard(false, 22, UiPalette.CardAltBackground);
            packageCard.ShowShadow = false;
            packageCard.Location = new Point(22, 178);
            packageCard.Size = new Size(308, 174);
            packageCard.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            packageCard.Padding = new Padding(16);
            card.Controls.Add(packageCard);
            BindControlWidth(card, packageCard, 22, 22, 260);

            Label packageTitle = CreateSectionEyebrow("内置字体包", UiPalette.AccentTextSoft);
            packageTitle.Location = new Point(16, 12);
            packageCard.Controls.Add(packageTitle);

            _packageInfoLabel = CreateMutedLabel("当前预设对应的字体包信息会显示在这里。", 16, 36, 356, 124);
            _packageInfoLabel.Size = new Size(276, 104);
            _packageInfoLabel.Font = new Font(Font.FontFamily, 9.8f, FontStyle.Regular);
            packageCard.Controls.Add(_packageInfoLabel);
            BindWrappedLabel(packageCard, _packageInfoLabel, 16, 36, 16, 220, 86);

            FlowLayoutPanel resources = new FlowLayoutPanel();
            resources.AutoSize = true;
            resources.BackColor = Color.Transparent;
            resources.Location = new Point(22, 370);
            resources.MaximumSize = new Size(308, 0);
            resources.WrapContents = true;
            card.Controls.Add(resources);
            BindFlowWidth(card, resources, 22, 22, 240);

            resources.Controls.Add(BuildButton("字体包目录", OpenFontPackagesButton_Click, ModernButtonStyle.Ghost, 0));
            resources.Controls.Add(BuildButton("预设目录", OpenPresetsButton_Click, ModernButtonStyle.Ghost, 0));
            resources.Controls.Add(BuildButton("备份目录", OpenBackupsButton_Click, ModernButtonStyle.Ghost, 0));

            return card;
        }

        private Control BuildMappingCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Top;
            card.Height = 454;
            card.Margin = new Padding(0, 0, 0, 12);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("替换映射预览");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            Label sub = CreateMutedLabel("下面只展示最关键的一批字体映射，方便确认整体风格有没有跑偏。", 22, 54, 760, 20);
            card.Controls.Add(sub);

            _mappingFlow = new FlowLayoutPanel();
            _mappingFlow.Location = new Point(22, 92);
            _mappingFlow.Size = new Size(840, 326);
            _mappingFlow.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _mappingFlow.AutoScroll = false;
            _mappingFlow.WrapContents = true;
            _mappingFlow.BackColor = Color.Transparent;
            card.Controls.Add(_mappingFlow);
            BindControlWidth(card, sub, 22, 22, 300);
            BindControlWidth(card, _mappingFlow, 22, 22, 320);

            return card;
        }

        private Control BuildRenderingPage()
        {
            TableLayoutPanel canvas;
            Panel page = CreatePage(out canvas);

            canvas.Controls.Add(BuildPageIntro("字体微调", "这里专门看渲染参数和样张，方便确认正文字体观感。"), 0, 0);

            TableLayoutPanel mainRow = new TableLayoutPanel();
            mainRow.Dock = DockStyle.Top;
            mainRow.Height = 520;
            mainRow.Margin = new Padding(0, 0, 0, 18);
            mainRow.BackColor = Color.Transparent;
            mainRow.ColumnCount = 2;
            mainRow.RowCount = 1;
            mainRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 42f));
            mainRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 58f));
            Control configCard = BuildRenderingConfigCard();
            Control previewCard = BuildRenderingPreviewCard();
            mainRow.Controls.Add(configCard, 0, 0);
            mainRow.Controls.Add(previewCard, 1, 0);
            ConfigureResponsiveRow(mainRow, configCard, previewCard, 1320, 540, 900, 42f, 58f);
            canvas.Controls.Add(mainRow, 0, 1);

            return page;
        }

        private Control BuildRenderingConfigCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Fill;
            card.Margin = new Padding(0, 0, 10, 0);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("当前渲染参数");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            Label sub = CreateMutedLabel("这里展示的是当前预设会写入的关键参数。", 22, 54, 320, 20);
            card.Controls.Add(sub);

            TableLayoutPanel grid = new TableLayoutPanel();
            grid.Location = new Point(22, 92);
            grid.Size = new Size(320, 232);
            grid.BackColor = Color.Transparent;
            grid.RowCount = 2;
            grid.ColumnCount = 2;
            grid.RowStyles.Add(new RowStyle(SizeType.Percent, 50f));
            grid.RowStyles.Add(new RowStyle(SizeType.Percent, 50f));
            grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50f));
            grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50f));
            card.Controls.Add(grid);

            grid.Controls.Add(CreateMiniMetricCard("渲染模式", out _renderModeValueLabel), 0, 0);
            grid.Controls.Add(CreateMiniMetricCard("Gamma", out _renderGammaValueLabel), 1, 0);
            grid.Controls.Add(CreateMiniMetricCard("边缘对比", out _renderContrastValueLabel), 0, 1);
            grid.Controls.Add(CreateMiniMetricCard("窗口字重", out _renderWeightValueLabel), 1, 1);

            ModernCardPanel faceCard = CreateCard(false, 22, UiPalette.CardAltBackground);
            faceCard.ShowShadow = false;
            faceCard.Location = new Point(22, 356);
            faceCard.Size = new Size(320, 56);
            faceCard.Padding = new Padding(16, 10, 16, 10);
            card.Controls.Add(faceCard);

            Label faceTitle = CreateSectionEyebrow("窗口字体", UiPalette.TextSecondary);
            faceTitle.Location = new Point(16, 8);
            faceCard.Controls.Add(faceTitle);

            _renderFaceValueLabel = new Label();
            _renderFaceValueLabel.AutoSize = false;
            _renderFaceValueLabel.BackColor = Color.Transparent;
            _renderFaceValueLabel.Location = new Point(16, 26);
            _renderFaceValueLabel.Size = new Size(288, 22);
            _renderFaceValueLabel.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            _renderFaceValueLabel.ForeColor = UiPalette.TextPrimary;
            _renderFaceValueLabel.AutoEllipsis = true;
            faceCard.Controls.Add(_renderFaceValueLabel);
            BindWrappedLabel(card, sub, 22, 54, 22, 240, 20);
            BindControlWidth(card, faceCard, 22, 22, 240);
            BindControlWidth(faceCard, _renderFaceValueLabel, 16, 16, 180);

            return card;
        }

        private Control BuildRenderingPreviewCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Fill;
            card.Margin = new Padding(10, 0, 0, 0);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("并排样张");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            Label sub = CreateMutedLabel("左边是当前系统消息字体基准，右边是当前预设的目标效果。", 22, 54, 430, 20);
            card.Controls.Add(sub);
            BindWrappedLabel(card, sub, 22, 54, 22, 280, 20);

            TableLayoutPanel split = new TableLayoutPanel();
            split.Location = new Point(22, 92);
            split.Size = new Size(420, 318);
            split.BackColor = Color.Transparent;
            split.ColumnCount = 2;
            split.RowCount = 1;
            split.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50f));
            split.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50f));
            card.Controls.Add(split);

            Control currentPane = BuildSamplePane("当前系统样张", false, out _renderCurrentBodyLabel);
            Control tunedPane = BuildSamplePane("当前预设样张", true, out _renderTunedBodyLabel);
            split.Controls.Add(currentPane, 0, 0);
            split.Controls.Add(tunedPane, 1, 0);
            BindControlWidth(card, split, 22, 22, 340);
            ConfigureResponsivePreviewSplit(split, currentPane, tunedPane, 980, 318, 430);

            _renderPreviewMetaLabel = CreateMutedLabel("这里只帮你确认字重、字宽和正文字体观感。", 22, 420, 420, 36);
            card.Controls.Add(_renderPreviewMetaLabel);
            BindWrappedLabel(card, _renderPreviewMetaLabel, 22, 420, 22, 280, 28);

            return card;
        }

        private Control BuildSettingsPage()
        {
            TableLayoutPanel canvas;
            Panel page = CreatePage(out canvas);

            canvas.Controls.Add(BuildPageIntro("设置与备份", "这里集中放运行参数、资源目录和系统快照。"), 0, 0);

            TableLayoutPanel topRow = new TableLayoutPanel();
            topRow.Dock = DockStyle.Top;
            topRow.Height = 356;
            topRow.Margin = new Padding(0, 0, 0, 18);
            topRow.BackColor = Color.Transparent;
            topRow.ColumnCount = 2;
            topRow.RowCount = 1;
            topRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 52f));
            topRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 48f));
            Control settingsCard = BuildSettingsCard();
            Control resourcesCard = BuildResourcesCard();
            topRow.Controls.Add(settingsCard, 0, 0);
            topRow.Controls.Add(resourcesCard, 1, 0);
            ConfigureResponsiveRow(topRow, settingsCard, resourcesCard, 1280, 372, 660, 52f, 48f);
            canvas.Controls.Add(topRow, 0, 1);

            canvas.Controls.Add(BuildBackupsCard(), 0, 2);
            canvas.Controls.Add(BuildLogCard(), 0, 3);

            return page;
        }

        private Control BuildSettingsCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Fill;
            card.Margin = new Padding(0, 0, 10, 0);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("应用行为");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            _settingsVersionLabel = CreateBadgeLabel("当前版本", UiPalette.AccentSoft, UiPalette.AccentTextSoft);
            _settingsVersionLabel.Location = new Point(22, 58);
            card.Controls.Add(_settingsVersionLabel);

            Label adminTitle = CreateSectionEyebrow("管理员状态", UiPalette.TextSecondary);
            adminTitle.Location = new Point(22, 102);
            card.Controls.Add(adminTitle);

            _settingsAdminLabel = CreateMutedLabel("正在读取权限状态…", 22, 122, 400, 42);
            _settingsAdminLabel.Size = new Size(360, 60);
            _settingsAdminLabel.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            card.Controls.Add(_settingsAdminLabel);

            _settingsRebuildCacheCheckBox = new CheckBox();
            _settingsRebuildCacheCheckBox.AutoSize = true;
            _settingsRebuildCacheCheckBox.BackColor = Color.Transparent;
            _settingsRebuildCacheCheckBox.Checked = true;
            _settingsRebuildCacheCheckBox.ForeColor = UiPalette.TextSecondary;
            _settingsRebuildCacheCheckBox.Location = new Point(22, 184);
            _settingsRebuildCacheCheckBox.Text = "重建字体缓存";
            _settingsRebuildCacheCheckBox.CheckedChanged += SettingsOptionChanged;
            card.Controls.Add(_settingsRebuildCacheCheckBox);

            _settingsRestartExplorerCheckBox = new CheckBox();
            _settingsRestartExplorerCheckBox.AutoSize = true;
            _settingsRestartExplorerCheckBox.BackColor = Color.Transparent;
            _settingsRestartExplorerCheckBox.Checked = true;
            _settingsRestartExplorerCheckBox.ForeColor = UiPalette.TextSecondary;
            _settingsRestartExplorerCheckBox.Location = new Point(22, 212);
            _settingsRestartExplorerCheckBox.Text = "应用后重启资源管理器";
            _settingsRestartExplorerCheckBox.CheckedChanged += SettingsOptionChanged;
            card.Controls.Add(_settingsRestartExplorerCheckBox);

            _settingsUpdateLabel = CreateMutedLabel("更新信息会显示在这里。", 22, 244, 400, 42);
            _settingsUpdateLabel.Size = new Size(360, 42);
            _settingsUpdateLabel.Font = new Font(Font.FontFamily, 9.8f, FontStyle.Regular);
            card.Controls.Add(_settingsUpdateLabel);
            BindWrappedLabel(card, _settingsAdminLabel, 22, 122, 22, 240, 40);
            BindWrappedLabel(card, _settingsUpdateLabel, 22, 244, 22, 240, 24);

            FlowLayoutPanel actions = new FlowLayoutPanel();
            actions.AutoSize = true;
            actions.BackColor = Color.Transparent;
            actions.Location = new Point(22, 286);
            actions.MaximumSize = new Size(360, 0);
            actions.WrapContents = true;
            card.Controls.Add(actions);
            BindFlowWidth(card, actions, 22, 22, 240);

            _settingsCheckUpdatesButton = BuildButton("检查更新", CheckUpdatesButton_Click, ModernButtonStyle.Primary, 0);
            actions.Controls.Add(_settingsCheckUpdatesButton);
            actions.Controls.Add(BuildButton("发布页", OpenUpdatePageButton_Click, ModernButtonStyle.Secondary, 0));
            actions.Controls.Add(BuildButton("仓库", OpenRepositoryButton_Click, ModernButtonStyle.Ghost, 0));

            return card;
        }

        private Control BuildResourcesCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Fill;
            card.Margin = new Padding(10, 0, 0, 0);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("资源目录");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            Label sub = CreateMutedLabel("字体包、预设和快照都能从这里直接打开。", 22, 54, 300, 20);
            card.Controls.Add(sub);

            FlowLayoutPanel actions = new FlowLayoutPanel();
            actions.AutoSize = true;
            actions.BackColor = Color.Transparent;
            actions.Location = new Point(22, 96);
            actions.MaximumSize = new Size(300, 0);
            actions.WrapContents = true;
            card.Controls.Add(actions);
            BindWrappedLabel(card, sub, 22, 54, 22, 220, 20);
            BindFlowWidth(card, actions, 22, 22, 220);

            actions.Controls.Add(BuildButton("字体目录", OpenFontPackagesButton_Click, ModernButtonStyle.Secondary, 0));
            actions.Controls.Add(BuildButton("预设目录", OpenPresetsButton_Click, ModernButtonStyle.Secondary, 0));
            actions.Controls.Add(BuildButton("备份目录", OpenBackupsButton_Click, ModernButtonStyle.Secondary, 0));
            actions.Controls.Add(BuildButton("发布页", OpenReleasePageButton_Click, ModernButtonStyle.Ghost, 0));

            return card;
        }

        private Control BuildBackupsCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Top;
            card.Height = 336;
            card.Margin = new Padding(0, 0, 0, 18);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("系统快照");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            _settingsBackupHintLabel = CreateMutedLabel("最近的备份记录会显示在这里。", 22, 54, 760, 20);
            card.Controls.Add(_settingsBackupHintLabel);
            BindWrappedLabel(card, _settingsBackupHintLabel, 22, 54, 22, 300, 20);

            FlowLayoutPanel actions = new FlowLayoutPanel();
            actions.AutoSize = true;
            actions.BackColor = Color.Transparent;
            actions.Location = new Point(22, 88);
            actions.MaximumSize = new Size(640, 0);
            actions.WrapContents = true;
            card.Controls.Add(actions);
            BindFlowWidth(card, actions, 22, 22, 300);

            actions.Controls.Add(BuildButton("新建备份", BackupButton_Click, ModernButtonStyle.Primary, 0));
            actions.Controls.Add(BuildButton("恢复最近", RestoreButton_Click, ModernButtonStyle.Secondary, 0));
            actions.Controls.Add(BuildButton("恢复默认", ResetWindowsDefaultsButton_Click, ModernButtonStyle.Secondary, 0));
            actions.Controls.Add(BuildButton("修复字体", RepairSystemFontsButton_Click, ModernButtonStyle.Ghost, 0));

            _backupListPanel = new FlowLayoutPanel();
            _backupListPanel.Location = new Point(22, 140);
            _backupListPanel.Size = new Size(1048, 160);
            _backupListPanel.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _backupListPanel.AutoScroll = true;
            _backupListPanel.WrapContents = false;
            _backupListPanel.FlowDirection = FlowDirection.TopDown;
            _backupListPanel.BackColor = Color.Transparent;
            _backupListPanel.Resize += BackupListPanel_Resize;
            card.Controls.Add(_backupListPanel);
            BindControlWidth(card, _backupListPanel, 22, 22, 300);

            return card;
        }

        private Control BuildLogCard()
        {
            ModernCardPanel card = CreateCard(false, 28, UiPalette.CardBackground);
            card.Dock = DockStyle.Top;
            card.Height = 242;
            card.Margin = new Padding(0, 0, 0, 12);
            card.Padding = new Padding(22);

            Label title = CreateSectionTitle("操作日志");
            title.Location = new Point(22, 20);
            card.Controls.Add(title);

            Label sub = CreateMutedLabel("应用预设、安装字体、恢复快照和检查更新的结果都会记在这里。", 22, 54, 740, 20);
            card.Controls.Add(sub);

            _clearLogButton = BuildButton("清空日志", ClearLogButton_Click, ModernButtonStyle.Ghost, 92);
            _clearLogButton.Location = new Point(972, 16);
            _clearLogButton.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _clearLogButton.Margin = new Padding(0);
            card.Controls.Add(_clearLogButton);

            ModernCardPanel logSurface = CreateCard(false, 22, Color.FromArgb(246, 249, 252));
            logSurface.ShowShadow = false;
            logSurface.Location = new Point(22, 88);
            logSurface.Size = new Size(1048, 132);
            logSurface.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            logSurface.Padding = new Padding(12);
            card.Controls.Add(logSurface);

            _logTextBox = new TextBox();
            _logTextBox.Multiline = true;
            _logTextBox.ScrollBars = ScrollBars.Vertical;
            _logTextBox.ReadOnly = true;
            _logTextBox.BorderStyle = BorderStyle.None;
            _logTextBox.Dock = DockStyle.Fill;
            _logTextBox.BackColor = Color.FromArgb(246, 249, 252);
            _logTextBox.ForeColor = UiPalette.TextPrimary;
            _logTextBox.Font = new Font("Consolas", 9.3f, FontStyle.Regular);
            logSurface.Controls.Add(_logTextBox);

            return card;
        }

        private Control CreateStatCard(string labelText, string noteText, out Label valueLabel, Color badgeBackColor, Color badgeForeColor)
        {
            ModernCardPanel card = CreateCard(false, 24, UiPalette.CardBackground);
            card.Dock = DockStyle.Fill;
            card.Padding = new Padding(16);
            card.ShowShadow = false;

            Label badge = CreateBadgeLabel(labelText, badgeBackColor, badgeForeColor);
            badge.Location = new Point(16, 14);
            card.Controls.Add(badge);

            valueLabel = CreateMutedLabel("--", 16, 48, 190, 34);
            valueLabel.Font = new Font(Font.FontFamily, 13.8f, FontStyle.Bold);
            valueLabel.ForeColor = UiPalette.TextPrimary;
            card.Controls.Add(valueLabel);

            Label note = CreateMutedLabel(noteText, 16, 86, 190, 26);
            card.Controls.Add(note);
            BindWrappedLabel(card, note, 16, 86, 16, 120, 24);

            return card;
        }

        private Control CreateMiniMetricCard(string titleText, out Label valueLabel)
        {
            ModernCardPanel card = CreateCard(false, 20, UiPalette.CardAltBackground);
            card.Dock = DockStyle.Fill;
            card.Padding = new Padding(14);
            card.ShowShadow = false;

            Label title = CreateSectionEyebrow(titleText, UiPalette.TextSecondary);
            title.Location = new Point(14, 14);
            card.Controls.Add(title);

            valueLabel = CreateMutedLabel("--", 14, 40, 150, 42);
            valueLabel.Font = new Font(Font.FontFamily, 12.2f, FontStyle.Bold);
            valueLabel.ForeColor = UiPalette.TextPrimary;
            card.Controls.Add(valueLabel);
            BindWrappedLabel(card, valueLabel, 14, 40, 14, 100, 36);

            return card;
        }

        private Control BuildSamplePane(string titleText, bool emphasize, out Label bodyLabel)
        {
            ModernCardPanel pane = CreateCard(false, 22, emphasize ? Color.FromArgb(247, 250, 255) : Color.FromArgb(250, 251, 253));
            pane.Dock = DockStyle.Fill;
            pane.ShowShadow = false;
            pane.Padding = new Padding(16);

            Label title = CreateSectionEyebrow(titleText, emphasize ? UiPalette.AccentTextSoft : UiPalette.TextSecondary);
            title.Location = new Point(16, 12);
            pane.Controls.Add(title);

            bodyLabel = CreateMutedLabel("敏捷的棕色狐狸\r\nAa 0123456789\r\nThe quick brown fox.", 16, 40, 150, 180);
            bodyLabel.Font = new Font(Font.FontFamily, 11.2f, emphasize ? FontStyle.Bold : FontStyle.Regular);
            bodyLabel.ForeColor = UiPalette.TextPrimary;
            pane.Controls.Add(bodyLabel);
            BindWrappedLabel(pane, bodyLabel, 16, 40, 16, 120, 120);

            return pane;
        }

        private Control BuildPageIntro(string titleText, string description)
        {
            Panel intro = new Panel();
            intro.Dock = DockStyle.Top;
            intro.Height = 118;
            intro.Margin = new Padding(0, 0, 0, 18);
            intro.BackColor = Color.Transparent;

            Label title = new Label();
            title.AutoSize = false;
            title.BackColor = Color.Transparent;
            title.Font = new Font(Font.FontFamily, 17.2f, FontStyle.Bold);
            title.ForeColor = UiPalette.TextPrimary;
            title.Location = new Point(0, 0);
            title.Size = new Size(760, 40);
            title.AutoEllipsis = true;
            title.Text = titleText;
            intro.Controls.Add(title);
            BindControlWidth(intro, title, 0, 0, 320);

            Label sub = CreateMutedLabel(description, 0, 52, 760, 36);
            intro.Controls.Add(sub);
            BindWrappedLabel(intro, sub, 0, 52, 0, 320, 20);

            return intro;
        }

        private void AppShellForm_Load(object sender, EventArgs e)
        {
            LoadFontPackages();
            LoadPresets();
            SyncOptionStates(true);
            RefreshVersionLabels();
            RefreshAdminState();
            RefreshPresetDetails();
            RefreshBackupList();

            Log("启动完成，已载入预设目录：" + Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Presets"));
            Log("备份目录：" + _service.BackupRoot);
            Log("当前版本：" + FormatVersion(GetCurrentVersion()));
        }

        private async void AppShellForm_Shown(object sender, EventArgs e)
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

            _fontPackages.Clear();

            foreach (FontPackage package in packages.Where(function => function != null && !string.IsNullOrWhiteSpace(function.Id)))
            {
                _fontPackages[package.Id] = package;
            }
        }

        private void LoadPresets()
        {
            string presetDirectory = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Presets");
            JavaScriptSerializer serializer = new JavaScriptSerializer();
            List<FontPreset> presets = new List<FontPreset>();

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

            _presets.Clear();
            _presets.AddRange(presets.OrderBy(function => function.Name));

            if (_presetComboBox == null)
            {
                return;
            }

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

        private void RefreshVersionLabels()
        {
            string versionText = "v" + FormatVersion(GetCurrentVersion());

            if (_topVersionLabel != null)
            {
                _topVersionLabel.Text = versionText;
            }

            if (_dashboardVersionBadgeLabel != null)
            {
                _dashboardVersionBadgeLabel.Text = "当前版本 " + versionText;
            }

            if (_settingsVersionLabel != null)
            {
                _settingsVersionLabel.Text = "当前版本 " + versionText;
            }
        }

        private void RefreshAdminState()
        {
            bool isAdmin = _service.IsAdministrator();
            string text = isAdmin
                ? "管理员模式已启用，可直接应用和备份。"
                : "当前不是管理员模式，应用前请右键以管理员身份运行。";
            Color color = isAdmin ? UiPalette.Success : UiPalette.Warning;

            if (_sidebarStatusLabel != null)
            {
                _sidebarStatusLabel.Text = isAdmin ? "管理员模式已启用" : "当前不是管理员模式";
                _sidebarStatusLabel.ForeColor = color;
            }

            if (_settingsAdminLabel != null)
            {
                _settingsAdminLabel.Text = text;
                _settingsAdminLabel.ForeColor = color;
            }

            if (_replacementAdminLabel != null)
            {
                _replacementAdminLabel.Text = text;
                _replacementAdminLabel.ForeColor = color;
            }

            if (_dashboardAdminValueLabel != null)
            {
                _dashboardAdminValueLabel.Text = isAdmin ? "已就绪" : "需提权";
                _dashboardAdminValueLabel.ForeColor = color;
            }
        }

        private FontPreset GetSelectedPreset()
        {
            return _presetComboBox == null ? null : _presetComboBox.SelectedItem as FontPreset;
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
                if (_descriptionLabel != null)
                {
                    _descriptionLabel.Text = "还没有读取到任何预设。";
                }

                if (_fontHintLabel != null)
                {
                    _fontHintLabel.Text = "请选择一个预设。";
                }

                if (_packageInfoLabel != null)
                {
                    _packageInfoLabel.Text = "当前没有可用的内置字体包信息。";
                }

                if (_dashboardPresetValueLabel != null)
                {
                    _dashboardPresetValueLabel.Text = "--";
                }

                if (_dashboardMissingValueLabel != null)
                {
                    _dashboardMissingValueLabel.Text = "--";
                }

                if (_dashboardPackageNoteLabel != null)
                {
                    _dashboardPackageNoteLabel.Text = "当前没有可展示的字体包。";
                }

                if (_installFontsButton != null)
                {
                    _installFontsButton.Enabled = false;
                }

                RefreshMappingPreview(null);
                RefreshRenderingPreview(null, null, new List<string>());
                return;
            }

            IList<string> missingFonts = _service.GetMissingFonts(preset);
            FontPackage package = GetSelectedFontPackage();

            if (_descriptionLabel != null)
            {
                _descriptionLabel.Text = string.IsNullOrWhiteSpace(preset.Description)
                    ? "这个预设暂时没有额外说明。"
                    : preset.Description;
            }

            if (_fontHintLabel != null)
            {
                _fontHintLabel.Text = missingFonts.Count == 0
                    ? "所需字体已齐：" + JoinDisplayList(preset.RequiredFonts)
                    : "还缺这些字体：" + string.Join("、", missingFonts);
            }

            if (_packageInfoLabel != null)
            {
                _packageInfoLabel.Text = BuildPackageSummary(package);
            }

            if (_installFontsButton != null)
            {
                _installFontsButton.Enabled = package != null;
            }

            RefreshDashboard(preset, package, missingFonts);
            RefreshMappingPreview(preset);
            RefreshRenderingPreview(preset, package, missingFonts);
        }

        private void RefreshDashboard(FontPreset preset, FontPackage package, IList<string> missingFonts)
        {
            string compactPresetName = GetCompactPresetName(preset);

            if (_dashboardPresetValueLabel != null)
            {
                _dashboardPresetValueLabel.Text = compactPresetName;
            }

            if (_dashboardMissingValueLabel != null)
            {
                _dashboardMissingValueLabel.Text = missingFonts.Count == 0 ? "已齐" : (missingFonts.Count + " 项");
                _dashboardMissingValueLabel.ForeColor = missingFonts.Count == 0 ? UiPalette.Success : UiPalette.Warning;
            }

            if (_dashboardHeroSupportLabel != null)
            {
                _dashboardHeroSupportLabel.Text =
                    "当前选中「" + compactPresetName + "」，" +
                    (missingFonts.Count == 0 ? "字体已齐，可以直接应用。" : "还缺 " + missingFonts.Count + " 款字体，先安装再应用。");
            }

            if (_dashboardPreviewTitleLabel != null)
            {
                _dashboardPreviewTitleLabel.Text = "敏捷的棕色狐狸 0123456789";
                _dashboardPreviewTitleLabel.Font = CreatePreviewFont(preset, 17.6f);
            }

            if (_dashboardPreviewBodyLabel != null)
            {
                _dashboardPreviewBodyLabel.Text =
                    "Aa The quick brown fox jumps over the lazy dog." + Environment.NewLine +
                    "资源管理器、桌面图标和常见正文区域都会尽量往同一套风格靠。";
                _dashboardPreviewBodyLabel.Font = CreatePreviewFont(preset, 10.2f, false);
            }

            if (_dashboardPreviewMetaLabel != null)
            {
                string faceName = preset.WindowMetrics == null ? "未设置" : SafeText(preset.WindowMetrics.FaceName, "未设置");
                string weight = preset.WindowMetrics == null ? "--" : preset.WindowMetrics.Weight.ToString();
                _dashboardPreviewMetaLabel.Text = "窗口字体：" + faceName + " ｜ 字重：" + weight;
            }

            if (_dashboardPackageNoteLabel != null)
            {
                _dashboardPackageNoteLabel.Text = package == null
                    ? "当前预设没有内置字体包，或者字体包目录不存在。"
                    : package.Name + Environment.NewLine + SafeText(package.Description, "暂无说明。");
            }
        }

        private void RefreshMappingPreview(FontPreset preset)
        {
            if (_mappingFlow == null)
            {
                return;
            }

            _mappingFlow.SuspendLayout();
            _mappingFlow.Controls.Clear();

            if (preset == null || preset.FontSubstitutes == null || preset.FontSubstitutes.Count == 0)
            {
                _mappingFlow.Controls.Add(CreateEmptyStateCard("当前预设没有字体映射信息。"));
                _mappingFlow.ResumeLayout();
                return;
            }

            foreach (KeyValuePair<string, string> pair in preset.FontSubstitutes.OrderBy(function => function.Key).Take(8))
            {
                _mappingFlow.Controls.Add(CreateMappingPreviewCard(pair.Key, pair.Value));
            }

            _mappingFlow.ResumeLayout();
        }

        private Control CreateMappingPreviewCard(string sourceName, string targetName)
        {
            ModernCardPanel card = CreateCard(false, 20, UiPalette.CardAltBackground);
            card.ShowShadow = false;
            card.BorderColor = UiPalette.Border;
            card.Size = new Size(156, 92);
            card.Margin = new Padding(0, 0, 10, 10);
            card.Padding = new Padding(14);

            Label sourceLabel = CreateSectionEyebrow("原始字体", UiPalette.TextSecondary);
            sourceLabel.Location = new Point(14, 12);
            card.Controls.Add(sourceLabel);

            Label sourceValue = CreateMutedLabel(sourceName, 14, 30, 126, 20);
            sourceValue.Font = new Font(Font.FontFamily, 9.6f, FontStyle.Bold);
            sourceValue.ForeColor = UiPalette.TextPrimary;
            sourceValue.AutoEllipsis = true;
            card.Controls.Add(sourceValue);

            Label arrow = CreateMutedLabel("→", 14, 56, 16, 18);
            arrow.Font = new Font(Font.FontFamily, 9.2f, FontStyle.Bold);
            arrow.ForeColor = UiPalette.TextMuted;
            card.Controls.Add(arrow);

            Label targetValue = CreateMutedLabel(targetName, 34, 54, 106, 22);
            targetValue.Font = new Font(Font.FontFamily, 9.4f, FontStyle.Bold);
            targetValue.ForeColor = UiPalette.AccentTextSoft;
            targetValue.AutoEllipsis = true;
            card.Controls.Add(targetValue);

            return card;
        }

        private void RefreshRenderingPreview(FontPreset preset, FontPackage package, IList<string> missingFonts)
        {
            if (preset == null)
            {
                return;
            }

            if (_renderModeValueLabel != null)
            {
                _renderModeValueLabel.Text = DescribeRenderingMode(preset);
            }

            if (_renderGammaValueLabel != null)
            {
                _renderGammaValueLabel.Text = preset.Rendering == null ? "--" : preset.Rendering.GammaLevel.ToString();
            }

            if (_renderContrastValueLabel != null)
            {
                _renderContrastValueLabel.Text = preset.Rendering == null ? "--" : ("Level " + preset.Rendering.TextContrastLevel);
            }

            if (_renderWeightValueLabel != null)
            {
                _renderWeightValueLabel.Text = preset.WindowMetrics == null ? "--" : preset.WindowMetrics.Weight.ToString();
            }

            if (_renderFaceValueLabel != null)
            {
                _renderFaceValueLabel.Text = preset.WindowMetrics == null ? "未设置" : SafeText(preset.WindowMetrics.FaceName, "未设置");
            }

            if (_renderCurrentBodyLabel != null)
            {
                _renderCurrentBodyLabel.Font = UiTypography.Create(12f, FontStyle.Regular);
                _renderCurrentBodyLabel.Text = "敏捷的棕色狐狸\r\nAa 0123456789\r\nThe quick brown fox.";
            }

            if (_renderTunedBodyLabel != null)
            {
                _renderTunedBodyLabel.Font = CreatePreviewFont(preset, 12.8f);
                _renderTunedBodyLabel.Text = "敏捷的棕色狐狸\r\nAa 0123456789\r\nThe quick brown fox.";
            }

            if (_renderPreviewMetaLabel != null)
            {
                _renderPreviewMetaLabel.Text =
                    "当前方案：" + SafeText(preset.Name, "未命名预设") +
                    " ｜ " +
                    (missingFonts.Count == 0 ? "所需字体已齐" : "仍缺 " + missingFonts.Count + " 款字体") +
                    (package == null ? string.Empty : " ｜ 字体包：" + package.Name);
            }
        }

        private void RefreshBackupList()
        {
            if (_backupListPanel == null)
            {
                return;
            }

            _backupListPanel.SuspendLayout();
            _backupListPanel.Controls.Clear();

            if (!Directory.Exists(_service.BackupRoot))
            {
                _settingsBackupHintLabel.Text = "还没有任何快照。第一次应用预设或点“新建备份”之后，这里就会有记录。";
                _backupListPanel.Controls.Add(CreateEmptyStateCard("还没有备份记录。"));
                _backupListPanel.ResumeLayout();
                return;
            }

            List<DirectoryInfo> backups = new DirectoryInfo(_service.BackupRoot)
                .GetDirectories()
                .OrderByDescending(function => function.CreationTimeUtc)
                .Take(6)
                .ToList();

            if (backups.Count == 0)
            {
                _settingsBackupHintLabel.Text = "还没有任何快照。第一次应用预设或点“新建备份”之后，这里就会有记录。";
                _backupListPanel.Controls.Add(CreateEmptyStateCard("还没有备份记录。"));
                _backupListPanel.ResumeLayout();
                return;
            }

            _settingsBackupHintLabel.Text = "最近 " + backups.Count + " 条快照，可直接恢复或打开目录。";

            foreach (DirectoryInfo backup in backups)
            {
                _backupListPanel.Controls.Add(CreateBackupCard(backup));
            }

            AdjustBackupCardWidths();
            _backupListPanel.ResumeLayout();
        }

        private Control CreateBackupCard(DirectoryInfo backup)
        {
            ModernCardPanel card = CreateCard(false, 20, UiPalette.CardAltBackground);
            card.ShowShadow = false;
            card.Width = 1000;
            card.Height = 70;
            card.Margin = new Padding(0, 0, 0, 10);
            card.Padding = new Padding(16);

            Label title = CreateMutedLabel(backup.Name, 16, 12, 300, 20);
            title.Font = new Font(Font.FontFamily, 10f, FontStyle.Bold);
            title.ForeColor = UiPalette.TextPrimary;
            card.Controls.Add(title);

            Label date = CreateMutedLabel(backup.CreationTime.ToString("yyyy-MM-dd HH:mm:ss"), 16, 36, 240, 18);
            card.Controls.Add(date);

            ModernButton restoreButton = BuildButton("恢复", RestoreSpecificBackupButton_Click, ModernButtonStyle.Secondary, 78);
            restoreButton.Tag = backup.FullName;
            restoreButton.Location = new Point(card.Width - 180, 16);
            restoreButton.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            restoreButton.Margin = new Padding(0);
            card.Controls.Add(restoreButton);

            ModernButton openButton = BuildButton("打开目录", OpenSpecificBackupButton_Click, ModernButtonStyle.Ghost, 94);
            openButton.Tag = backup.FullName;
            openButton.Location = new Point(card.Width - 94, 16);
            openButton.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            openButton.Margin = new Padding(0);
            card.Controls.Add(openButton);

            return card;
        }

        private Control CreateEmptyStateCard(string text)
        {
            ModernCardPanel card = CreateCard(false, 20, UiPalette.CardAltBackground);
            card.ShowShadow = false;
            card.Width = 1000;
            card.Height = 70;
            card.Margin = new Padding(0, 0, 0, 10);
            card.Padding = new Padding(16);

            Label body = CreateMutedLabel(text, 16, 22, 940, 22);
            body.Font = new Font(Font.FontFamily, 9.8f, FontStyle.Regular);
            card.Controls.Add(body);

            return card;
        }

        private async Task CheckForUpdatesAsync(bool silent)
        {
            Version currentVersion = GetCurrentVersion();
            SetUpdateButtonsEnabled(false);
            SetUpdateStatus("正在检查更新，请稍等…", UiPalette.TextSecondary);
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
                    Log("发现新版本：" + SafeText(result.LatestTag, "未知标签"));

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
                SetUpdateStatus("更新检查失败，可以稍后重试。", UiPalette.Warning);
                Log("检查更新失败：" + ex.Message);

                if (!silent)
                {
                    MessageBox.Show(this, "检查更新失败：" + ex.Message, "检查更新", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                }
            }
            finally
            {
                SetUpdateButtonsEnabled(true);
            }
        }

        private void ApplyUpdateStatus(UpdateCheckResult result)
        {
            if (result == null)
            {
                SetUpdateStatus("暂时没有拿到版本信息。", UiPalette.TextSecondary);

                if (_dashboardUpdateValueLabel != null)
                {
                    _dashboardUpdateValueLabel.Text = "未获取";
                    _dashboardUpdateValueLabel.ForeColor = UiPalette.TextSecondary;
                }

                if (_openDownloadButton != null)
                {
                    _openDownloadButton.Visible = false;
                }

                return;
            }

            if (result.IsUpdateAvailable)
            {
                string published = result.PublishedAt.HasValue
                    ? "，发布于 " + result.PublishedAt.Value.ToLocalTime().ToString("yyyy-MM-dd")
                    : string.Empty;
                string summary = GetReleaseSummary(result.ReleaseBody);
                string text = "发现新版本 " + SafeText(result.LatestTag, "未知标签") + published;

                if (!string.IsNullOrWhiteSpace(summary))
                {
                    text += Environment.NewLine + summary;
                }

                SetUpdateStatus(text, UiPalette.AccentTextSoft);

                if (_dashboardUpdateValueLabel != null)
                {
                    _dashboardUpdateValueLabel.Text = "有新版本";
                    _dashboardUpdateValueLabel.ForeColor = UiPalette.Accent;
                }

                if (_openDownloadButton != null)
                {
                    _openDownloadButton.Visible = true;
                    _openDownloadButton.Text = "下载 " + SafeText(result.LatestTag, "新版本");
                }

                return;
            }

            SetUpdateStatus("当前已经是最新版本 " + FormatVersion(result.CurrentVersion) + "。", UiPalette.Success);

            if (_dashboardUpdateValueLabel != null)
            {
                _dashboardUpdateValueLabel.Text = "已最新";
                _dashboardUpdateValueLabel.ForeColor = UiPalette.Success;
            }

            if (_openDownloadButton != null)
            {
                _openDownloadButton.Visible = false;
            }
        }

        private void ReplacementOptionChanged(object sender, EventArgs e)
        {
            SyncOptionStates(false);
        }

        private void SettingsOptionChanged(object sender, EventArgs e)
        {
            SyncOptionStates(true);
        }

        private void SyncOptionStates(bool fromSettings)
        {
            if (fromSettings)
            {
                if (_rebuildCacheCheckBox != null && _settingsRebuildCacheCheckBox != null && _rebuildCacheCheckBox.Checked != _settingsRebuildCacheCheckBox.Checked)
                {
                    _rebuildCacheCheckBox.Checked = _settingsRebuildCacheCheckBox.Checked;
                }

                if (_restartExplorerCheckBox != null && _settingsRestartExplorerCheckBox != null && _restartExplorerCheckBox.Checked != _settingsRestartExplorerCheckBox.Checked)
                {
                    _restartExplorerCheckBox.Checked = _settingsRestartExplorerCheckBox.Checked;
                }
            }
            else
            {
                if (_rebuildCacheCheckBox != null && _settingsRebuildCacheCheckBox != null && _settingsRebuildCacheCheckBox.Checked != _rebuildCacheCheckBox.Checked)
                {
                    _settingsRebuildCacheCheckBox.Checked = _rebuildCacheCheckBox.Checked;
                }

                if (_restartExplorerCheckBox != null && _settingsRestartExplorerCheckBox != null && _settingsRestartExplorerCheckBox.Checked != _restartExplorerCheckBox.Checked)
                {
                    _settingsRestartExplorerCheckBox.Checked = _restartExplorerCheckBox.Checked;
                }
            }
        }

        private bool ShouldRebuildFontCache()
        {
            if (_settingsRebuildCacheCheckBox != null)
            {
                return _settingsRebuildCacheCheckBox.Checked;
            }

            return _rebuildCacheCheckBox != null && _rebuildCacheCheckBox.Checked;
        }

        private bool ShouldRestartExplorer()
        {
            if (_settingsRestartExplorerCheckBox != null)
            {
                return _settingsRestartExplorerCheckBox.Checked;
            }

            return _restartExplorerCheckBox != null && _restartExplorerCheckBox.Checked;
        }

        private void BackupListPanel_Resize(object sender, EventArgs e)
        {
            AdjustBackupCardWidths();
        }

        private void AdjustBackupCardWidths()
        {
            if (_backupListPanel == null)
            {
                return;
            }

            int width = Math.Max(320, _backupListPanel.ClientSize.Width - SystemInformation.VerticalScrollBarWidth - 6);

            foreach (Control control in _backupListPanel.Controls)
            {
                if (control.Width != width)
                {
                    control.Width = width;
                }
            }
        }

        private void SetUpdateButtonsEnabled(bool enabled)
        {
            if (_checkUpdatesButton != null)
            {
                _checkUpdatesButton.Enabled = enabled;
            }

            if (_settingsCheckUpdatesButton != null)
            {
                _settingsCheckUpdatesButton.Enabled = enabled;
            }
        }

        private void SetUpdateStatus(string text, Color color)
        {
            if (_dashboardUpdateStatusLabel != null)
            {
                _dashboardUpdateStatusLabel.Text = text;
                _dashboardUpdateStatusLabel.ForeColor = color;
            }

            if (_settingsUpdateLabel != null)
            {
                _settingsUpdateLabel.Text = text;
                _settingsUpdateLabel.ForeColor = color;
            }
        }

        private static string BuildPackageSummary(FontPackage package)
        {
            if (package == null)
            {
                return "当前预设没有可用的内置字体包。";
            }

            return package.Name + Environment.NewLine + SafeText(package.Description, "暂无说明。");
        }

        private static string DescribeRenderingMode(FontPreset preset)
        {
            if (preset == null || preset.Rendering == null)
            {
                return "--";
            }

            if (preset.Rendering.ClearTypeLevel == 0 || preset.Rendering.PixelStructure == 0)
            {
                return "灰阶抗锯齿";
            }

            return "ClearType";
        }

        private static string JoinDisplayList(IList<string> items)
        {
            return items == null || items.Count == 0 ? "未提供" : string.Join("、", items);
        }

        private static string SafeText(string value, string fallback)
        {
            return string.IsNullOrWhiteSpace(value) ? fallback : value;
        }

        private static string GetCompactPresetName(FontPreset preset)
        {
            string name = SafeText(preset == null ? null : preset.Name, "未命名预设");
            name = name.Replace("统一预设", string.Empty).Trim();
            return name.Length == 0 ? "未命名预设" : name;
        }

        private Font CreatePreviewFont(FontPreset preset, float size)
        {
            return CreatePreviewFont(preset, size, true);
        }

        private Font CreatePreviewFont(FontPreset preset, float size, bool allowBold)
        {
            string faceName = preset != null && preset.WindowMetrics != null
                ? preset.WindowMetrics.FaceName
                : UiTypography.DefaultFamilyName;
            FontStyle style = allowBold && preset != null && preset.WindowMetrics != null && preset.WindowMetrics.Weight >= 550
                ? FontStyle.Bold
                : FontStyle.Regular;

            try
            {
                return new Font(faceName, size, style);
            }
            catch
            {
                return UiTypography.Create(size, style);
            }
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

                if (trimmed.Length > 66)
                {
                    trimmed = trimmed.Substring(0, 66) + "...";
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
                : UpdateService.LatestReleasePage;

            OpenUrl(url);
        }

        private void OpenDownloadButton_Click(object sender, EventArgs e)
        {
            string url = _latestUpdateResult != null && !string.IsNullOrWhiteSpace(_latestUpdateResult.DownloadUrl)
                ? _latestUpdateResult.DownloadUrl
                : UpdateService.LatestReleasePage;

            OpenUrl(url);
        }

        private void OpenReleasePageButton_Click(object sender, EventArgs e)
        {
            OpenUrl(UpdateService.LatestReleasePage);
        }

        private void OpenRepositoryButton_Click(object sender, EventArgs e)
        {
            OpenUrl(UpdateService.RepositoryPage);
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
                        string.Join(Environment.NewLine, missing) + Environment.NewLine + Environment.NewLine +
                        "是否先安装软件内置字体包再继续？" + Environment.NewLine + Environment.NewLine +
                        "是：先安装再继续" + Environment.NewLine +
                        "否：直接继续应用" + Environment.NewLine +
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
                        string.Join(Environment.NewLine, missing) + Environment.NewLine + Environment.NewLine +
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
                    ShouldRebuildFontCache(),
                    ShouldRestartExplorer());

                Log("已应用预设：" + SafeText(preset.Name, "未命名预设"));
                Log("已创建备份：" + backup);
                RefreshPresetDetails();
                RefreshBackupList();
            });
        }

        private void BackupButton_Click(object sender, EventArgs e)
        {
            RunAction("创建备份", delegate
            {
                string backup = _service.CreateBackup();
                Log("已创建备份：" + backup);
                RefreshBackupList();
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
                    ShouldRebuildFontCache(),
                    ShouldRestartExplorer());

                Log("已恢复备份：" + restored);
                RefreshPresetDetails();
                RefreshBackupList();
            });
        }

        private void ResetWindowsDefaultsButton_Click(object sender, EventArgs e)
        {
            DialogResult confirm = MessageBox.Show(
                this,
                "这会先自动创建一份当前快照，然后执行以下恢复动作：" + Environment.NewLine + Environment.NewLine +
                "1. 清除本工具写入的字体替换项" + Environment.NewLine +
                "2. 把窗口字体和文字渲染恢复到 Windows 默认风格" + Environment.NewLine +
                "3. 按你的设置重建字体缓存并重启资源管理器" + Environment.NewLine + Environment.NewLine +
                "如果系统自带字体文件已经被删掉，这一步只能恢复设置，字体文件本身请再点“修复系统字体”。" + Environment.NewLine + Environment.NewLine +
                "确定继续吗？",
                "恢复 Windows 默认",
                MessageBoxButtons.YesNo,
                MessageBoxIcon.Question);

            if (confirm != DialogResult.Yes)
            {
                return;
            }

            RunAction("恢复 Windows 默认", delegate
            {
                string backup = _service.ResetToWindowsDefaults(
                    ShouldRebuildFontCache(),
                    ShouldRestartExplorer());

                Log("已恢复 Windows 默认字体设置。");
                Log("已创建备份：" + backup);
                RefreshPresetDetails();
                RefreshBackupList();
            });
        }

        private void RepairSystemFontsButton_Click(object sender, EventArgs e)
        {
            DialogResult confirm = MessageBox.Show(
                this,
                "这会打开管理员命令窗口，并按微软官方建议依次运行：" + Environment.NewLine + Environment.NewLine +
                "DISM.exe /Online /Cleanup-Image /RestoreHealth" + Environment.NewLine +
                "sfc /scannow" + Environment.NewLine + Environment.NewLine +
                "它适合用来修复被删掉或损坏的系统字体文件，过程可能比较久，也可能会用到 Windows Update。" + Environment.NewLine + Environment.NewLine +
                "现在开始吗？",
                "修复系统字体",
                MessageBoxButtons.YesNo,
                MessageBoxIcon.Question);

            if (confirm != DialogResult.Yes)
            {
                return;
            }

            RunAction("启动系统字体修复", delegate
            {
                _service.LaunchSystemFontRepair();
                Log("已启动 Windows 官方修复：DISM /RestoreHealth + sfc /scannow。");
            });
        }

        private void RestoreSpecificBackupButton_Click(object sender, EventArgs e)
        {
            Control control = sender as Control;
            string backupPath = control == null ? null : control.Tag as string;

            if (string.IsNullOrWhiteSpace(backupPath))
            {
                return;
            }

            DialogResult confirm = MessageBox.Show(
                this,
                "要恢复这个备份并刷新资源管理器吗？" + Environment.NewLine + Environment.NewLine + backupPath,
                "恢复指定备份",
                MessageBoxButtons.YesNo,
                MessageBoxIcon.Question);

            if (confirm != DialogResult.Yes)
            {
                return;
            }

            RunAction("恢复指定备份", delegate
            {
                _service.RestoreBackup(
                    backupPath,
                    ShouldRebuildFontCache(),
                    ShouldRestartExplorer());

                Log("已恢复备份：" + backupPath);
                RefreshPresetDetails();
                RefreshBackupList();
            });
        }

        private void OpenSpecificBackupButton_Click(object sender, EventArgs e)
        {
            Control control = sender as Control;
            string backupPath = control == null ? null : control.Tag as string;
            _service.OpenDirectory(backupPath);
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
            if (_logTextBox != null)
            {
                _logTextBox.Clear();
            }
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
                int backdrop = NativeMethods.DWMSBT_MAINWINDOW;
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

        private enum AppPage
        {
            Dashboard,
            Replacement,
            Rendering,
            Settings
        }

        private static class NativeMethods
        {
            public const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
            public const int DWMWA_SYSTEMBACKDROP_TYPE = 38;
            public const int DWMWCP_ROUND = 2;
            public const int DWMSBT_MAINWINDOW = 2;

            [DllImport("dwmapi.dll")]
            public static extern int DwmSetWindowAttribute(IntPtr hwnd, int dwAttribute, ref int pvAttribute, int cbAttribute);
        }
    }
}
