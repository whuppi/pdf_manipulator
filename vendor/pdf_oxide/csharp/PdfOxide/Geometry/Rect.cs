using System;

namespace PdfOxide.Geometry
{
    /// <summary>
    /// Represents a rectangle with position and dimensions in PDF points.
    /// </summary>
    public struct Rect : IEquatable<Rect>
    {
        /// <summary>
        /// Gets the left (X) coordinate.
        /// </summary>
        public float X { get; }

        /// <summary>
        /// Gets the top (Y) coordinate.
        /// </summary>
        public float Y { get; }

        /// <summary>
        /// Gets the width.
        /// </summary>
        public float Width { get; }

        /// <summary>
        /// Gets the height.
        /// </summary>
        public float Height { get; }

        /// <summary>
        /// Initializes a new instance of the Rect struct.
        /// </summary>
        public Rect(float x, float y, float width, float height)
        {
            X = x;
            Y = y;
            Width = width;
            Height = height;
        }

        /// <summary>
        /// Gets the right edge (X + Width).
        /// </summary>
        public float Right => X + Width;

        /// <summary>
        /// Gets the bottom edge (Y + Height).
        /// </summary>
        public float Bottom => Y + Height;

        /// <summary>
        /// Gets the area (Width × Height).
        /// </summary>
        public float Area => Width * Height;

        /// <summary>
        /// Checks if this rectangle contains the given point.
        /// </summary>
        public bool Contains(Point point) =>
            point.X >= X && point.X <= Right &&
            point.Y >= Y && point.Y <= Bottom;

        /// <summary>
        /// Checks if this rectangle intersects with another rectangle.
        /// </summary>
        public bool Intersects(Rect other) =>
            !(Right < other.X || X > other.Right ||
              Bottom < other.Y || Y > other.Bottom);

        public override bool Equals(object? obj) => obj is Rect rect && Equals(rect);

        public bool Equals(Rect other) =>
            X == other.X && Y == other.Y && Width == other.Width && Height == other.Height;

        public override int GetHashCode()
        {
            unchecked
            {
                int hash = 17;
                hash = hash * 31 + X.GetHashCode();
                hash = hash * 31 + Y.GetHashCode();
                hash = hash * 31 + Width.GetHashCode();
                hash = hash * 31 + Height.GetHashCode();
                return hash;
            }
        }

        public override string ToString() =>
            $"Rect(X={X}, Y={Y}, Width={Width}, Height={Height})";

        public static bool operator ==(Rect left, Rect right) => left.Equals(right);

        public static bool operator !=(Rect left, Rect right) => !left.Equals(right);
    }
}
