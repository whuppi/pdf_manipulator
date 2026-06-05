using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// Horizontal alignment options for text placement primitives
    /// (<see cref="PageBuilder.TextInRect"/>) and table columns
    /// (<see cref="Column"/>).
    /// </summary>
    /// <remarks>
    /// The underlying FFI encodes these as <c>int</c>:
    /// <c>0 = Left</c>, <c>1 = Center</c>, <c>2 = Right</c>. Keep the
    /// enum discriminant values stable with <c>src/ffi.rs</c>.
    /// </remarks>
    public enum Alignment
    {
        /// <summary>Left-align the text (default).</summary>
        Left = 0,

        /// <summary>Center the text within the available width.</summary>
        Center = 1,

        /// <summary>Right-align the text.</summary>
        Right = 2,
    }

    /// <summary>
    /// Declarative column spec for buffered and streaming tables
    /// (<see cref="PageBuilder.Table(TableSpec)"/>,
    /// <see cref="PageBuilder.StreamingTable(System.Collections.Generic.IReadOnlyList{Column}, bool)"/>).
    /// </summary>
    /// <remarks>
    /// <para>
    /// Each column carries a display <see cref="Header"/> (used when the
    /// owning table has <c>HasHeader = true</c>), a <see cref="Width"/>
    /// in PDF points, and an <see cref="Align"/> specifier for both the
    /// header cell and every body cell in that column.
    /// </para>
    /// <para>
    /// Column widths are absolute — percent or fractional widths are not
    /// currently supported at the FFI layer (tracked for a later
    /// release). To mimic percentage layout, compute
    /// <c>pageWidth * pct</c> in user code.
    /// </para>
    /// </remarks>
    public sealed class Column
    {
        /// <summary>Header label shown when the table has <c>HasHeader = true</c>.</summary>
        public string Header { get; set; }

        /// <summary>Column width in PDF points.</summary>
        public float Width { get; set; }

        /// <summary>Alignment for the header cell and every body cell.</summary>
        public Alignment Align { get; set; }

        /// <summary>
        /// Construct a column.
        /// </summary>
        /// <param name="header">Header label; pass an empty string if
        /// the owning table sets <c>HasHeader = false</c>.</param>
        /// <param name="width">Column width in PDF points. Must be positive.</param>
        /// <param name="align">Horizontal alignment (default
        /// <see cref="Alignment.Left"/>).</param>
        public Column(string header, float width, Alignment align = Alignment.Left)
        {
            ArgumentNullException.ThrowIfNull(header);
            if (width <= 0f)
                throw new ArgumentOutOfRangeException(nameof(width), "width must be positive");
            Header = header;
            Width = width;
            Align = align;
        }
    }

    /// <summary>
    /// Buffered table specification — all rows are materialised up
    /// front and passed to the FFI in a single call via
    /// <see cref="PageBuilder.Table(TableSpec)"/>.
    /// </summary>
    /// <remarks>
    /// Memory is <c>O(rows * columns)</c>. For large data sets prefer
    /// <see cref="PageBuilder.StreamingTable(System.Collections.Generic.IReadOnlyList{Column}, bool)"/>,
    /// which presents a streaming <c>AddRow</c> surface (the v0.3.39
    /// implementation still buffers internally — see
    /// <see cref="StreamingTable"/>).
    /// </remarks>
    public sealed class TableSpec
    {
        /// <summary>Ordered columns. Must be non-empty at <see cref="PageBuilder.Table(TableSpec)"/>.</summary>
        public List<Column> Columns { get; set; } = new();

        /// <summary>
        /// Row-major cell matrix. Each inner collection must have
        /// exactly <c>Columns.Count</c> entries. <see langword="null"/>
        /// cells are rendered as empty strings.
        /// </summary>
        public IList<IList<string>> Rows { get; set; } = new List<IList<string>>();

        /// <summary>
        /// When <see langword="true"/>, the first rendered row is drawn
        /// as a header (bold + default header background). When
        /// <see langword="false"/>, all rows are body rows and
        /// <see cref="Column.Header"/> labels are ignored.
        /// </summary>
        public bool HasHeader { get; set; }
    }

    /// <summary>
    /// Column-sizing strategy for streaming tables (issue #400).
    /// </summary>
    public abstract class TableMode
    {
        private TableMode() { }

        /// <summary>Use the <see cref="Column.Width"/> from each column as-is (default).</summary>
        public sealed class Fixed : TableMode { }

        /// <summary>
        /// Buffer the first <see cref="SampleRows"/> rows, measure the
        /// maximum content width per column, then freeze widths for the
        /// remainder of the stream.
        /// </summary>
        public sealed class Sample : TableMode
        {
            /// <summary>Number of rows to sample (default 20).</summary>
            public int SampleRows { get; init; } = 20;
            /// <summary>Minimum column width in PDF points (default 0).</summary>
            public float MinColWidthPt { get; init; } = 0f;
            /// <summary>Maximum column width in PDF points (default 9999).</summary>
            public float MaxColWidthPt { get; init; } = 9999f;
        }
    }

    /// <summary>
    /// Streaming-table handle returned by
    /// <see cref="PageBuilder.StreamingTable(System.Collections.Generic.IReadOnlyList{Column}, bool, TableMode)"/>.
    /// Accepts rows one at a time via <see cref="AddRow(string[])"/>
    /// and finalises on <see cref="Build"/>.
    /// </summary>
    /// <remarks>
    /// Always wrap in a <c>using</c>. <see cref="Dispose"/> on an
    /// un-<see cref="Build"/>t handle finishes the streaming table
    /// without drawing any rows — useful for error recovery mid-stream.
    /// </remarks>
    public sealed class StreamingTable : IDisposable
    {
        private readonly PageBuilder _page;
        private readonly int _nCols;
        private bool _built;
        private bool _disposed;

        /// <summary>
        /// Number of rows pushed since the last batch boundary.
        /// Backed by the Rust FFI layer — no C# buffering.
        /// </summary>
        public int PendingRowCount =>
            (int)NativeMethods.PdfPageBuilderStreamingTablePendingRowCount(_page.InternalHandle);

        /// <summary>Number of complete batches recorded by the native layer so far.</summary>
        public int BatchCount =>
            (int)NativeMethods.PdfPageBuilderStreamingTableBatchCount(_page.InternalHandle);

        internal unsafe StreamingTable(
            PageBuilder page,
            IReadOnlyList<Column> columns,
            bool repeatHeader,
            TableMode? mode,
            int maxRowspan,
            int batchSize = 256)
        {
            _page = page;
            _nCols = columns.Count;

            // Encode headers, widths, aligns.
            var headers = new byte[_nCols][];
            var widths = new float[_nCols];
            var aligns = new int[_nCols];
            for (int i = 0; i < _nCols; i++)
            {
                headers[i] = Encoding.UTF8.GetBytes((columns[i].Header ?? string.Empty) + "\0");
                widths[i] = columns[i].Width;
                aligns[i] = (int)columns[i].Align;
            }

            int modeInt = 0;
            int sampleRows = 20;
            float minW = 0f, maxW = 9999f;
            if (mode is TableMode.Sample s)
            {
                modeInt = 1;
                sampleRows = s.SampleRows;
                minW = s.MinColWidthPt;
                maxW = s.MaxColWidthPt;
            }

            var hHandles = new GCHandle[_nCols];
            var hPtrs = new IntPtr[_nCols];
            GCHandle hPtrsH = default, widthsH = default, alignsH = default;
            try
            {
                for (int i = 0; i < _nCols; i++)
                {
                    hHandles[i] = GCHandle.Alloc(headers[i], GCHandleType.Pinned);
                    hPtrs[i] = hHandles[i].AddrOfPinnedObject();
                }
                hPtrsH = GCHandle.Alloc(hPtrs, GCHandleType.Pinned);
                widthsH = GCHandle.Alloc(widths, GCHandleType.Pinned);
                alignsH = GCHandle.Alloc(aligns, GCHandleType.Pinned);

                NativeMethods.PdfPageBuilderStreamingTableBeginV2(
                    page.InternalHandle,
                    (nuint)_nCols,
                    (byte**)hPtrsH.AddrOfPinnedObject(),
                    (float*)widthsH.AddrOfPinnedObject(),
                    (int*)alignsH.AddrOfPinnedObject(),
                    repeatHeader ? 1 : 0,
                    modeInt,
                    (nuint)sampleRows,
                    minW, maxW,
                    (nuint)Math.Max(1, maxRowspan),
                    out var ec);
                ExceptionMapper.ThrowIfError(ec);

                NativeMethods.PdfPageBuilderStreamingTableSetBatchSize(
                    page.InternalHandle,
                    (nuint)Math.Max(1, batchSize),
                    out ec);
                ExceptionMapper.ThrowIfError(ec);
            }
            finally
            {
                if (hPtrsH.IsAllocated) hPtrsH.Free();
                if (widthsH.IsAllocated) widthsH.Free();
                if (alignsH.IsAllocated) alignsH.Free();
                for (int i = 0; i < _nCols; i++)
                    if (hHandles[i].IsAllocated) hHandles[i].Free();
            }
        }

        /// <summary>
        /// Append a row. The <paramref name="cells"/> array must have
        /// exactly <c>Columns.Count</c> entries; <see langword="null"/>
        /// entries render as empty strings. All cells have rowspan=1.
        /// </summary>
        public unsafe StreamingTable AddRow(params string[] cells)
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            if (_built)
                throw new InvalidOperationException("StreamingTable already built.");
            ArgumentNullException.ThrowIfNull(cells);
            if (cells.Length != _nCols)
                throw new ArgumentException(
                    $"row width mismatch: got {cells.Length} cells, expected {_nCols}",
                    nameof(cells));

            var cellBytes = new byte[_nCols][];
            for (int i = 0; i < _nCols; i++)
                cellBytes[i] = Encoding.UTF8.GetBytes((cells[i] ?? string.Empty) + "\0");

            var handles = new GCHandle[_nCols];
            var ptrs = new IntPtr[_nCols];
            GCHandle ptrsH = default;
            try
            {
                for (int i = 0; i < _nCols; i++)
                {
                    handles[i] = GCHandle.Alloc(cellBytes[i], GCHandleType.Pinned);
                    ptrs[i] = handles[i].AddrOfPinnedObject();
                }
                ptrsH = GCHandle.Alloc(ptrs, GCHandleType.Pinned);
                NativeMethods.PdfPageBuilderStreamingTablePushRow(
                    _page.InternalHandle,
                    (nuint)_nCols,
                    (byte**)ptrsH.AddrOfPinnedObject(),
                    out var ec);
                ExceptionMapper.ThrowIfError(ec);
            }
            finally
            {
                if (ptrsH.IsAllocated) ptrsH.Free();
                for (int i = 0; i < _nCols; i++)
                    if (handles[i].IsAllocated) handles[i].Free();
            }
            return this;
        }

        /// <summary>
        /// Append a row with per-cell rowspan values. Each <c>(string Text, int Rowspan)</c>
        /// pair specifies the cell text and how many rows it spans (1 = normal).
        /// Requires <c>maxRowspan ≥ 2</c> in the <see cref="PageBuilder.StreamingTable"/> call.
        /// </summary>
        public unsafe StreamingTable AddRowSpan(params (string? Text, int Rowspan)[] cells)
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            if (_built)
                throw new InvalidOperationException("StreamingTable already built.");
            ArgumentNullException.ThrowIfNull(cells);
            if (cells.Length != _nCols)
                throw new ArgumentException(
                    $"row width mismatch: got {cells.Length} cells, expected {_nCols}",
                    nameof(cells));

            var cellBytes = new byte[_nCols][];
            var rowspans = new nuint[_nCols];
            for (int i = 0; i < _nCols; i++)
            {
                cellBytes[i] = Encoding.UTF8.GetBytes((cells[i].Text ?? string.Empty) + "\0");
                rowspans[i] = (nuint)Math.Max(1, cells[i].Rowspan);
            }

            var handles = new GCHandle[_nCols];
            var ptrs = new IntPtr[_nCols];
            GCHandle ptrsH = default, rowspansH = default;
            try
            {
                for (int i = 0; i < _nCols; i++)
                {
                    handles[i] = GCHandle.Alloc(cellBytes[i], GCHandleType.Pinned);
                    ptrs[i] = handles[i].AddrOfPinnedObject();
                }
                ptrsH = GCHandle.Alloc(ptrs, GCHandleType.Pinned);
                rowspansH = GCHandle.Alloc(rowspans, GCHandleType.Pinned);
                NativeMethods.PdfPageBuilderStreamingTablePushRowV2(
                    _page.InternalHandle,
                    (nuint)_nCols,
                    (byte**)ptrsH.AddrOfPinnedObject(),
                    (nuint*)rowspansH.AddrOfPinnedObject(),
                    out var ec);
                ExceptionMapper.ThrowIfError(ec);
            }
            finally
            {
                if (ptrsH.IsAllocated) ptrsH.Free();
                if (rowspansH.IsAllocated) rowspansH.Free();
                for (int i = 0; i < _nCols; i++)
                    if (handles[i].IsAllocated) handles[i].Free();
            }
            return this;
        }

        /// <summary>
        /// Explicitly mark a batch boundary in the native layer.
        /// Normally triggered automatically when the configured batch size is reached.
        /// </summary>
        public void Flush()
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            NativeMethods.PdfPageBuilderStreamingTableFlush(_page.InternalHandle, out var ec);
            ExceptionMapper.ThrowIfError(ec);
        }

        /// <summary>
        /// Close the streaming table and return the page for continued chaining.
        /// </summary>
        public PageBuilder Build()
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            if (_built)
                throw new InvalidOperationException("StreamingTable already built.");
            _built = true;
            NativeMethods.PdfPageBuilderStreamingTableFinish(_page.InternalHandle, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return _page;
        }

        /// <summary>
        /// Drop the handle. If <see cref="Build"/> has not been called the
        /// open streaming table is finished silently.
        /// </summary>
        public void Dispose()
        {
            if (_disposed) return;
            _disposed = true;
            if (!_built)
            {
                _built = true;
                try
                {
                    NativeMethods.PdfPageBuilderStreamingTableFinish(_page.InternalHandle, out _);
                }
                catch (ObjectDisposedException) { }
            }
        }
    }
}
