using System;

namespace PdfOxide.Geometry
{
    /// <summary>
    /// Represents a color in RGBA format (0-255 for each component).
    /// </summary>
    public struct Color : IEquatable<Color>
    {
        /// <summary>
        /// Gets or sets the red component (0-255).
        /// </summary>
        public byte Red { get; }

        /// <summary>
        /// Gets or sets the green component (0-255).
        /// </summary>
        public byte Green { get; }

        /// <summary>
        /// Gets or sets the blue component (0-255).
        /// </summary>
        public byte Blue { get; }

        /// <summary>
        /// Gets or sets the alpha (opacity) component (0-255, where 255 is fully opaque).
        /// </summary>
        public byte Alpha { get; }

        /// <summary>
        /// Initializes a new instance of the Color struct with RGB values (alpha = 255).
        /// </summary>
        public Color(byte red, byte green, byte blue)
            : this(red, green, blue, 255)
        {
        }

        /// <summary>
        /// Initializes a new instance of the Color struct with RGBA values.
        /// </summary>
        public Color(byte red, byte green, byte blue, byte alpha)
        {
            Red = red;
            Green = green;
            Blue = blue;
            Alpha = alpha;
        }

        /// <summary>
        /// Creates a color from a 32-bit ARGB value.
        /// </summary>
        public static Color FromArgb(uint argb)
        {
            var a = (byte)((argb >> 24) & 0xFF);
            var r = (byte)((argb >> 16) & 0xFF);
            var g = (byte)((argb >> 8) & 0xFF);
            var b = (byte)(argb & 0xFF);
            return new Color(r, g, b, a);
        }

        /// <summary>
        /// Gets the color as a 32-bit ARGB value.
        /// </summary>
        public uint ToArgb() =>
            ((uint)Alpha << 24) | ((uint)Red << 16) | ((uint)Green << 8) | Blue;

        /// <summary>
        /// Gets the color as a hexadecimal string (#RRGGBB).
        /// </summary>
        public string ToHex() =>
            $"#{Red:X2}{Green:X2}{Blue:X2}";

        /// <summary>
        /// Gets the opacity as a float (0.0 - 1.0).
        /// </summary>
        public float Opacity => Alpha / 255f;

        /// <summary>
        /// Predefined color: Black.
        /// </summary>
        public static Color Black => new Color(0, 0, 0);

        /// <summary>
        /// Predefined color: White.
        /// </summary>
        public static Color White => new Color(255, 255, 255);


        /// <summary>
        /// Predefined color: Yellow.
        /// </summary>
        public static Color Yellow => new Color(255, 255, 0);

        /// <summary>
        /// Predefined color: Cyan.
        /// </summary>
        public static Color Cyan => new Color(0, 255, 255);

        /// <summary>
        /// Predefined color: Magenta.
        /// </summary>
        public static Color Magenta => new Color(255, 0, 255);

        public override bool Equals(object? obj) => obj is Color color && Equals(color);

        public bool Equals(Color other) =>
            Red == other.Red && Green == other.Green && Blue == other.Blue && Alpha == other.Alpha;

        public override int GetHashCode()
        {
            unchecked
            {
                int hash = 17;
                hash = hash * 31 + Red.GetHashCode();
                hash = hash * 31 + Green.GetHashCode();
                hash = hash * 31 + Blue.GetHashCode();
                hash = hash * 31 + Alpha.GetHashCode();
                return hash;
            }
        }

        public override string ToString() =>
            $"Color(R={Red}, G={Green}, B={Blue}, A={Alpha})";

        public static bool operator ==(Color left, Color right) => left.Equals(right);

        public static bool operator !=(Color left, Color right) => !left.Equals(right);
    }
}
