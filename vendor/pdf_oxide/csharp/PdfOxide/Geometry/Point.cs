using System;

namespace PdfOxide.Geometry
{
    /// <summary>
    /// Represents a point with X and Y coordinates in PDF points.
    /// </summary>
    public struct Point : IEquatable<Point>
    {
        /// <summary>
        /// Gets the X coordinate.
        /// </summary>
        public float X { get; }

        /// <summary>
        /// Gets the Y coordinate.
        /// </summary>
        public float Y { get; }

        /// <summary>
        /// Initializes a new instance of the Point struct.
        /// </summary>
        public Point(float x, float y)
        {
            X = x;
            Y = y;
        }

        /// <summary>
        /// Gets the distance from this point to another point.
        /// </summary>
        public float Distance(Point other)
        {
            var dx = X - other.X;
            var dy = Y - other.Y;
            return (float)Math.Sqrt(dx * dx + dy * dy);
        }

        /// <summary>
        /// Gets the distance from the origin (0, 0).
        /// </summary>
        public float Magnitude => (float)Math.Sqrt(X * X + Y * Y);

        public override bool Equals(object? obj) => obj is Point point && Equals(point);

        public bool Equals(Point other) => X == other.X && Y == other.Y;

        public override int GetHashCode()
        {
            unchecked
            {
                int hash = 17;
                hash = hash * 31 + X.GetHashCode();
                hash = hash * 31 + Y.GetHashCode();
                return hash;
            }
        }

        public override string ToString() => $"Point(X={X}, Y={Y})";

        public static bool operator ==(Point left, Point right) => left.Equals(right);

        public static bool operator !=(Point left, Point right) => !left.Equals(right);

        public static Point operator +(Point left, Point right) =>
            new Point(left.X + right.X, left.Y + right.Y);

        public static Point operator -(Point left, Point right) =>
            new Point(left.X - right.X, left.Y - right.Y);

        public static Point operator *(Point point, float scalar) =>
            new Point(point.X * scalar, point.Y * scalar);

        public static Point operator /(Point point, float scalar) =>
            new Point(point.X / scalar, point.Y / scalar);
    }
}
