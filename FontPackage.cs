using System.Collections.Generic;

namespace WinGlow
{
    public sealed class FontPackage
    {
        public string Id { get; set; }
        public string DirectoryName { get; set; }
        public string Name { get; set; }
        public string Description { get; set; }
        public string RecommendedFor { get; set; }
        public string LicenseName { get; set; }
        public string LicenseUrl { get; set; }
        public string SourceUrl { get; set; }
        public List<string> RequiredFonts { get; set; }
        public List<FontPackageFile> Files { get; set; }
    }

    public sealed class FontPackageFile
    {
        public string RelativePath { get; set; }
        public string RegistryName { get; set; }
        public string InstalledFileName { get; set; }
    }
}
