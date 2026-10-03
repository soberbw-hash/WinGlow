using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Text;
using System.IO;
using System.Windows.Forms;

namespace WinGlow
{
    internal static class UiPalette
    {
        public static readonly Color WindowBackground = Color.FromArgb(244, 247, 252);
        public static readonly Color SidebarBackground = Color.FromArgb(249, 251, 254);
        public static readonly Color SidebarBorder = Color.FromArgb(225, 231, 239);
        public static readonly Color CardBackground = Color.FromArgb(255, 255, 255);
        public static readonly Color CardAltBackground = Color.FromArgb(249, 251, 254);
        public static readonly Color Border = Color.FromArgb(223, 229, 237);
        public static readonly Color BorderStrong = Color.FromArgb(210, 219, 230);
        public static readonly Color Accent = Color.FromArgb(37, 99, 235);
        public static readonly Color AccentHover = Color.FromArgb(29, 78, 216);
        public static readonly Color AccentPressed = Color.FromArgb(30, 64, 175);
        public static readonly Color AccentSoft = Color.FromArgb(233, 242, 255);
        public static readonly Color AccentSoftStrong = Color.FromArgb(220, 234, 255);
        public static readonly Color AccentTextSoft = Color.FromArgb(24, 73, 177);
        public static readonly Color TextPrimary = Color.FromArgb(19, 26, 38);
        public static readonly Color TextSecondary = Color.FromArgb(84, 98, 121);
        public static readonly Color TextMuted = Color.FromArgb(120, 132, 152);
        public static readonly Color Success = Color.FromArgb(10, 133, 88);
        public static readonly Color SuccessSoft = Color.FromArgb(231, 248, 240);
        public static readonly Color Warning = Color.FromArgb(191, 111, 20);
        public static readonly Color WarningSoft = Color.FromArgb(255, 245, 229);
        public static readonly Color Danger = Color.FromArgb(188, 63, 63);
        public static readonly Color DangerSoft = Color.FromArgb(252, 238, 238);
        public static readonly Color HeroStart = Color.FromArgb(236, 243, 255);
        public static readonly Color HeroEnd = Color.FromArgb(244, 249, 255);
    }

    internal static class UiTypography
    {
        private static readonly PrivateFontCollection PrivateFonts = new PrivateFontCollection();
        private static FontFamily _family;
        private static bool _initialized;

        public static FontFamily Family
        {
            get
            {
                EnsureInitialized();
                return _family;
            }
        }

        public static string DefaultFamilyName
        {
            get { return Family.Name; }
        }

        public static void Initialize(string baseDirectory)
        {
            if (_initialized)
            {
                return;
            }

            _initialized = true;

            try
            {
                string[] candidates =
                {
                    Path.Combine(baseDirectory, "FontPackages", "harmonyos-sc", "HarmonyOS_Sans_SC_Regular.ttf"),
                    Path.Combine(baseDirectory, "FontPackages", "harmonyos-sc", "HarmonyOS_Sans_SC_Medium.ttf"),
                    Path.Combine(baseDirectory, "FontPackages", "harmonyos-sc", "HarmonyOS_Sans_SC_Bold.ttf"),
                    Path.Combine(baseDirectory, "FontPackages", "sarasa-ui-sc", "SarasaUiSC-Regular.ttf"),
                    Path.Combine(baseDirectory, "FontPackages", "sarasa-ui-sc", "SarasaUiSC-SemiBold.ttf"),
                    Path.Combine(baseDirectory, "FontPackages", "sarasa-ui-sc", "SarasaUiSC-Bold.ttf")
                };

                foreach (string candidate in candidates)
                {
                    if (File.Exists(candidate))
                    {
                        PrivateFonts.AddFontFile(candidate);
                    }
                }

                if (PrivateFonts.Families.Length > 0)
                {
                    _family = Array.Find(PrivateFonts.Families, family => family.Name.IndexOf("HarmonyOS", StringComparison.OrdinalIgnoreCase) >= 0)
                        ?? Array.Find(PrivateFonts.Families, family => family.Name.IndexOf("Sarasa", StringComparison.OrdinalIgnoreCase) >= 0)
                        ?? PrivateFonts.Families[0];
                }
            }
            catch
            {
            }

            if (_family == null)
            {
                try
                {
                    _family = new FontFamily("Microsoft YaHei UI");
                }
                catch
                {
                    try
                    {
                        _family = new FontFamily("Segoe UI");
                    }
                    catch
                    {
                        _family = SystemFonts.MessageBoxFont.FontFamily;
                    }
                }
            }
        }

        public static Font Create(float size, FontStyle style = FontStyle.Regular)
        {
            return new Font(Family, size, style, GraphicsUnit.Point);
        }

        private static void EnsureInitialized()
        {
            if (_initialized)
            {
                return;
            }

            Initialize(AppDomain.CurrentDomain.BaseDirectory);
        }
    }

    internal static class UiGeometry
    {
        public static GraphicsPath CreateRoundedRectangle(Rectangle bounds, int radius)
        {
            GraphicsPath path = new GraphicsPath();

            if (radius <= 0)
            {
                path.AddRectangle(bounds);
                path.CloseFigure();
                return path;
            }

            int diameter = radius * 2;
            Rectangle arc = new Rectangle(bounds.Location, new Size(diameter, diameter));

            path.AddArc(arc, 180, 90);
            arc.X = bounds.Right - diameter;
            path.AddArc(arc, 270, 90);
            arc.Y = bounds.Bottom - diameter;
            path.AddArc(arc, 0, 90);
            arc.X = bounds.Left;
            path.AddArc(arc, 90, 90);
            path.CloseFigure();
            return path;
        }
    }

    public sealed class ModernCardPanel : Panel
    {
        private int _cornerRadius = 26;
        private bool _useGradient;
        private bool _showGlow;
        private bool _showShadow;
        private Color _fillColor = UiPalette.CardBackground;
        private Color _gradientStartColor = UiPalette.HeroStart;
        private Color _gradientEndColor = UiPalette.HeroEnd;
        private Color _borderColor = UiPalette.Border;

        public ModernCardPanel()
        {
            DoubleBuffered = true;
            ResizeRedraw = false;
            SetStyle(ControlStyles.SupportsTransparentBackColor, true);
            BackColor = Color.Transparent;
            Padding = new Padding(20);
        }

        public int CornerRadius
        {
            get { return _cornerRadius; }
            set
            {
                _cornerRadius = value;
                Invalidate();
            }
        }

        public bool UseGradient
        {
            get { return _useGradient; }
            set
            {
                _useGradient = value;
                Invalidate();
            }
        }

        public bool ShowGlow
        {
            get { return _showGlow; }
            set
            {
                _showGlow = value;
                Invalidate();
            }
        }

        public bool ShowShadow
        {
            get { return _showShadow; }
            set
            {
                _showShadow = value;
                Invalidate();
            }
        }

        public Color FillColor
        {
            get { return _fillColor; }
            set
            {
                _fillColor = value;
                Invalidate();
            }
        }

        public Color GradientStartColor
        {
            get { return _gradientStartColor; }
            set
            {
                _gradientStartColor = value;
                Invalidate();
            }
        }

        public Color GradientEndColor
        {
            get { return _gradientEndColor; }
            set
            {
                _gradientEndColor = value;
                Invalidate();
            }
        }

        public Color BorderColor
        {
            get { return _borderColor; }
            set
            {
                _borderColor = value;
                Invalidate();
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);

            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            Rectangle bounds = new Rectangle(0, 0, Width - 1, Height - 1);

            using (GraphicsPath path = UiGeometry.CreateRoundedRectangle(bounds, _cornerRadius))
            {
                if (_showShadow)
                {
                    Rectangle shadowBounds = new Rectangle(0, 10, Width - 1, Height - 11);
                    using (GraphicsPath shadowPath = UiGeometry.CreateRoundedRectangle(shadowBounds, _cornerRadius))
                    using (SolidBrush shadowBrush = new SolidBrush(Color.FromArgb(14, 15, 23, 42)))
                    {
                        e.Graphics.FillPath(shadowBrush, shadowPath);
                    }
                }

                if (_useGradient)
                {
                    using (LinearGradientBrush brush = new LinearGradientBrush(
                        bounds,
                        _gradientStartColor,
                        _gradientEndColor,
                        LinearGradientMode.ForwardDiagonal))
                    {
                        e.Graphics.FillPath(brush, path);
                    }
                }
                else
                {
                    using (SolidBrush brush = new SolidBrush(_fillColor))
                    {
                        e.Graphics.FillPath(brush, path);
                    }
                }

                if (_showGlow)
                {
                    using (SolidBrush glowBrush = new SolidBrush(Color.FromArgb(70, 255, 255, 255)))
                    {
                        e.Graphics.FillEllipse(glowBrush, new Rectangle(Width - 200, -20, 220, 220));
                    }

                    using (SolidBrush accentBrush = new SolidBrush(Color.FromArgb(34, UiPalette.Accent)))
                    {
                        e.Graphics.FillEllipse(accentBrush, new Rectangle(-45, Height - 118, 155, 155));
                    }
                }

                using (Pen border = new Pen(_borderColor))
                {
                    e.Graphics.DrawPath(border, path);
                }
            }
        }

    }

    public enum ModernButtonStyle
    {
        Primary,
        Secondary,
        Ghost
    }

    public sealed class ModernButton : Button
    {
        private ModernButtonStyle _buttonStyle;
        private bool _hovered;
        private bool _pressed;

        public ModernButton()
        {
            FlatStyle = FlatStyle.Flat;
            FlatAppearance.BorderSize = 0;
            TabStop = true;
            Cursor = Cursors.Hand;
            DoubleBuffered = true;
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint, true);
            Height = 38;
            Width = 132;
            Font = UiTypography.Create(9.3f, FontStyle.Regular);
            ForeColor = UiPalette.TextPrimary;
            BackColor = Color.Transparent;
        }

        public int CornerRadius { get; set; }

        public ModernButtonStyle ButtonStyle
        {
            get { return _buttonStyle; }
            set
            {
                _buttonStyle = value;
                Invalidate();
            }
        }

        protected override void OnMouseEnter(EventArgs e)
        {
            base.OnMouseEnter(e);
            _hovered = true;
            Invalidate();
        }

        protected override void OnMouseLeave(EventArgs e)
        {
            base.OnMouseLeave(e);
            _hovered = false;
            _pressed = false;
            Invalidate();
        }

        protected override void OnMouseDown(MouseEventArgs mevent)
        {
            base.OnMouseDown(mevent);
            _pressed = true;
            Invalidate();
        }

        protected override void OnMouseUp(MouseEventArgs mevent)
        {
            base.OnMouseUp(mevent);
            _pressed = false;
            Invalidate();
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            e.Graphics.Clear(ResolveSurfaceColor());

            Rectangle bounds = new Rectangle(0, 0, Width - 1, Height - 1);
            Color fill;
            Color border;
            Color textColor;

            ResolveColors(out fill, out border, out textColor);

            using (GraphicsPath path = UiGeometry.CreateRoundedRectangle(bounds, CornerRadius <= 0 ? Height / 2 : CornerRadius))
            using (SolidBrush brush = new SolidBrush(fill))
            using (Pen pen = new Pen(border))
            {
                e.Graphics.FillPath(brush, path);
                e.Graphics.DrawPath(pen, path);
            }

            TextRenderer.DrawText(
                e.Graphics,
                Text,
                Font,
                bounds,
                textColor,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPadding);
        }

        private void ResolveColors(out Color fill, out Color border, out Color textColor)
        {
            if (!Enabled)
            {
                fill = Color.FromArgb(241, 244, 248);
                border = fill;
                textColor = UiPalette.TextMuted;
                return;
            }

            if (_buttonStyle == ModernButtonStyle.Primary)
            {
                fill = _pressed ? UiPalette.AccentPressed : (_hovered ? UiPalette.AccentHover : UiPalette.Accent);
                border = fill;
                textColor = Color.White;
                return;
            }

            if (_buttonStyle == ModernButtonStyle.Ghost)
            {
                fill = _pressed
                    ? Color.FromArgb(236, 241, 247)
                    : (_hovered ? Color.FromArgb(245, 248, 252) : ResolveSurfaceColor());
                border = fill;
                textColor = UiPalette.TextSecondary;
                return;
            }

            fill = _pressed
                ? Color.FromArgb(242, 246, 251)
                : (_hovered ? Color.FromArgb(248, 250, 253) : UiPalette.CardBackground);
            border = fill;
            textColor = UiPalette.TextPrimary;
        }

        private Color ResolveSurfaceColor()
        {
            Control current = Parent;

            while (current != null)
            {
                ModernCardPanel panel = current as ModernCardPanel;
                if (panel != null)
                {
                    return panel.UseGradient ? panel.GradientEndColor : panel.FillColor;
                }

                if (current.BackColor.A > 0 && current.BackColor != Color.Transparent)
                {
                    return current.BackColor;
                }

                current = current.Parent;
            }

            return UiPalette.WindowBackground;
        }
    }

    public sealed class ModernNavButton : Button
    {
        private bool _hovered;
        private bool _pressed;
        private bool _selected;

        public ModernNavButton()
        {
            FlatStyle = FlatStyle.Flat;
            FlatAppearance.BorderSize = 0;
            TabStop = true;
            Cursor = Cursors.Hand;
            DoubleBuffered = true;
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint, true);
            Height = 42;
            Width = 180;
            Font = UiTypography.Create(9.4f, FontStyle.Bold);
            ForeColor = UiPalette.TextSecondary;
            BackColor = Color.Transparent;
            TextAlign = ContentAlignment.MiddleLeft;
            Padding = new Padding(30, 0, 12, 0);
        }

        public bool Selected
        {
            get { return _selected; }
            set
            {
                _selected = value;
                Invalidate();
            }
        }

        protected override void OnMouseEnter(EventArgs e)
        {
            base.OnMouseEnter(e);
            _hovered = true;
            Invalidate();
        }

        protected override void OnMouseLeave(EventArgs e)
        {
            base.OnMouseLeave(e);
            _hovered = false;
            _pressed = false;
            Invalidate();
        }

        protected override void OnMouseDown(MouseEventArgs mevent)
        {
            base.OnMouseDown(mevent);
            _pressed = true;
            Invalidate();
        }

        protected override void OnMouseUp(MouseEventArgs mevent)
        {
            base.OnMouseUp(mevent);
            _pressed = false;
            Invalidate();
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            e.Graphics.Clear(ResolveSurfaceColor());

            Rectangle bounds = new Rectangle(0, 0, Width - 1, Height - 1);
            Color fill = Color.Transparent;
            Color textColor = UiPalette.TextSecondary;
            Color dotColor = Color.FromArgb(170, UiPalette.TextMuted);

            if (_selected)
            {
                fill = UiPalette.AccentSoft;
                textColor = UiPalette.AccentTextSoft;
                dotColor = UiPalette.Accent;
            }
            else if (_pressed)
            {
                fill = Color.FromArgb(238, 243, 249);
                textColor = UiPalette.TextPrimary;
                dotColor = UiPalette.TextPrimary;
            }
            else if (_hovered)
            {
                fill = Color.FromArgb(245, 248, 251);
                textColor = UiPalette.TextPrimary;
                dotColor = UiPalette.TextSecondary;
            }

            using (GraphicsPath path = UiGeometry.CreateRoundedRectangle(bounds, 14))
            using (SolidBrush brush = new SolidBrush(fill))
            {
                if (fill.A > 0)
                {
                    e.Graphics.FillPath(brush, path);
                }
            }

            if (_selected)
            {
                using (SolidBrush accentBrush = new SolidBrush(UiPalette.Accent))
                {
                    e.Graphics.FillRoundedRectangle(accentBrush, new Rectangle(0, 9, 4, Height - 18), 4);
                }
            }

            using (SolidBrush dotBrush = new SolidBrush(dotColor))
            {
                e.Graphics.FillEllipse(dotBrush, new Rectangle(12, (Height - 8) / 2, 8, 8));
            }

            Rectangle textBounds = new Rectangle(28, 0, Width - 40, Height);
            TextRenderer.DrawText(
                e.Graphics,
                Text,
                Font,
                textBounds,
                textColor,
                TextFormatFlags.VerticalCenter | TextFormatFlags.Left | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPadding);
        }

        private Color ResolveSurfaceColor()
        {
            Control current = Parent;

            while (current != null)
            {
                if (current.BackColor.A > 0 && current.BackColor != Color.Transparent)
                {
                    return current.BackColor;
                }

                current = current.Parent;
            }

            return UiPalette.SidebarBackground;
        }
    }

    internal static class GraphicsExtensions
    {
        public static void FillRoundedRectangle(this Graphics graphics, Brush brush, Rectangle bounds, int radius)
        {
            using (GraphicsPath path = UiGeometry.CreateRoundedRectangle(bounds, radius))
            {
                graphics.FillPath(brush, path);
            }
        }
    }
}
