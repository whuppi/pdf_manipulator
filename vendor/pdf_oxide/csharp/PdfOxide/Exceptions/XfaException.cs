using System;

namespace PdfOxide.Exceptions
{
    /// <summary>
    /// Exception for XFA (XML Forms Architecture) form operation errors.
    ///
    /// <para>Raised when XFA form parsing fails, form conversion fails, or field access fails.</para>
    /// </summary>
    public class XfaException : PdfException
    {
        private const string XfaErrorCode = "9400";

        /// <summary>
        /// Initializes a new instance of the XfaException class with the specified message.
        /// </summary>
        /// <param name="message">The detail message</param>
        public XfaException(string message)
            : base(XfaErrorCode, "XFA operation failed: " + message)
        {
        }

        /// <summary>
        /// Initializes a new instance of the XfaException class with message and inner exception.
        /// </summary>
        /// <param name="message">The detail message</param>
        /// <param name="innerException">The cause</param>
        public XfaException(string message, Exception? innerException = null)
            : base(XfaErrorCode, "XFA operation failed: " + message, innerException)
        {
        }
    }
}
