using System;
using Microsoft.Win32.SafeHandles;

namespace PdfOxide.Internal
{
    /// <summary>
    /// Safe handle wrapper for native Rust pointers with automatic cleanup.
    /// Uses SafeHandle for guaranteed resource cleanup and thread safety.
    /// </summary>
    /// <remarks>
    /// This class ensures that native resources are properly released even if an exception occurs.
    /// It is derived from SafeHandleZeroOrMinusOneIsInvalid which treats zero and -1 as invalid handles.
    /// </remarks>
    public sealed class NativeHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private readonly Action<IntPtr>? _finalizer;

        /// <summary>
        /// Initializes a new instance of the <see cref="NativeHandle"/> class.
        /// </summary>
        /// <remarks>
        /// This constructor creates an invalid handle that can be set later.
        /// </remarks>
        public NativeHandle() : base(true)
        {
        }

        /// <summary>
        /// Initializes a new instance of the <see cref="NativeHandle"/> class with a handle and finalizer.
        /// </summary>
        /// <param name="handle">The native pointer.</param>
        /// <param name="finalizer">The cleanup action to call when the handle is released.</param>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="finalizer"/> is null.</exception>
        public NativeHandle(IntPtr handle, Action<IntPtr> finalizer) : base(true)
        {
            _finalizer = finalizer ?? throw new ArgumentNullException(nameof(finalizer));
            SetHandle(handle);
        }

        /// <summary>
        /// Gets the underlying pointer value.
        /// </summary>
        /// <value>The native pointer.</value>
        /// <exception cref="ObjectDisposedException">Thrown if the handle has been disposed.</exception>
        public IntPtr Ptr
        {
            get
            {
                ObjectDisposedException.ThrowIf(IsInvalid || IsClosed, this);
                return handle;
            }
        }

        /// <summary>
        /// Gets the raw underlying pointer value without lifetime checks.
        /// Useful for comparing against <see cref="IntPtr.Zero"/> and passing to free functions.
        /// </summary>
        /// <value>The native pointer, or <see cref="IntPtr.Zero"/> if invalid.</value>
        public IntPtr Value => handle;

        /// <summary>
        /// Releases the native handle.
        /// </summary>
        /// <returns>True if the handle was successfully released, false otherwise.</returns>
        protected override bool ReleaseHandle()
        {
            if (!IsInvalid && _finalizer != null)
            {
                try
                {
                    _finalizer(handle);
                }
                catch
                {
                    // Suppress finalizer exceptions but still report the handle as
                    // released — returning false from ReleaseHandle causes the runtime
                    // to retry finalization, which would loop forever for a finalizer
                    // that always throws.
                }
            }
            return true;
        }
    }
}
