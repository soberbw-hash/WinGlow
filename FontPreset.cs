using System.Collections.Generic;

namespace WinGlow
{
    public sealed class FontPreset
    {
        public string Name { get; set; }
        public string Description { get; set; }
        public string FontPackageId { get; set; }
        public List<string> RequiredFonts { get; set; }
        public Dictionary<string, string> FontSubstitutes { get; set; }
        public DesktopTextSettings DesktopTextSettings { get; set; }
        public RenderingSettings Rendering { get; set; }
        public WindowMetricsSettings WindowMetrics { get; set; }
        public string SourcePath { get; set; }

        public override string ToString()
        {
            return string.IsNullOrWhiteSpace(Name) ? "(未命名预设)" : Name;
        }
    }

    public sealed class DesktopTextSettings
    {
        public string FontSmoothing { get; set; }
        public int FontSmoothingType { get; set; }
        public int FontSmoothingGamma { get; set; }
        public int FontSmoothingOrientation { get; set; }
    }

    public sealed class RenderingSettings
    {
        public int PixelStructure { get; set; }
        public int GammaLevel { get; set; }
        public int ClearTypeLevel { get; set; }
        public int TextContrastLevel { get; set; }
    }

    public sealed class WindowMetricsSettings
    {
        public string FaceName { get; set; }
        public int Weight { get; set; }
        public int Quality { get; set; }
    }
}
