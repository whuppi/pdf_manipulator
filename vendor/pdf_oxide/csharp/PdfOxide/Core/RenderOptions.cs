using System;

namespace PdfOxide.Core
{
    /// <summary>
    /// Output format for <see cref="PdfDocument.RenderPage(int, RenderOptions)"/>.
    /// </summary>
    public enum RenderImageFormat
    {
        /// <summary>PNG with optional transparent background.</summary>
        Png = 0,
        /// <summary>JPEG; respects <see cref="RenderOptions.JpegQuality"/>.</summary>
        Jpeg = 1,
    }

    /// <summary>
    /// Options controlling how a page is rendered to an image.
    /// Mirrors Rust's <c>RenderOptions</c>
    /// (see <c>src/rendering/page_renderer.rs:41</c>).
    /// </summary>
    /// <remarks>
    /// Instances are mutable value-equivalent: create once, set what you need,
    /// pass to <see cref="PdfDocument.RenderPage(int, RenderOptions)"/>.
    ///
    /// Filed as gap B in the cross-binding API audit and reported by
    /// u/gevorgter on Reddit on 2026-04-21 ("I can't figure out how to do
    /// it with C#").
    /// </remarks>
    public sealed class RenderOptions
    {
        /// <summary>Resolution in dots per inch (default 150).</summary>
        public int Dpi { get; set; } = 150;

        /// <summary>Output image format (default PNG).</summary>
        public RenderImageFormat Format { get; set; } = RenderImageFormat.Png;

        /// <summary>
        /// Background fill colour in RGBA 0.0..=1.0. Defaults to opaque
        /// white. Set <see cref="TransparentBackground"/> to drop the fill
        /// entirely (PNG only, since JPEG has no alpha).
        /// </summary>
        public (float R, float G, float B, float A) Background { get; set; } = (1f, 1f, 1f, 1f);

        /// <summary>
        /// When true, no background fill is written. The output PNG will
        /// preserve alpha; JPEG will fall back to the default white.
        /// </summary>
        public bool TransparentBackground { get; set; }

        /// <summary>Whether to render annotation layer (default true).</summary>
        public bool RenderAnnotations { get; set; } = true;

        /// <summary>
        /// JPEG quality, 1..=100. Only applied when <see cref="Format"/>
        /// is <see cref="RenderImageFormat.Jpeg"/>. Default 85.
        /// </summary>
        public int JpegQuality { get; set; } = 85;

        internal void Validate()
        {
            if (Dpi <= 0)
                throw new ArgumentException($"Dpi must be > 0, got {Dpi}", nameof(Dpi));
            if (Format == RenderImageFormat.Jpeg && (JpegQuality is < 1 or > 100))
                throw new ArgumentException(
                    $"JpegQuality must be in 1..=100, got {JpegQuality}", nameof(JpegQuality));
        }
    }
}
