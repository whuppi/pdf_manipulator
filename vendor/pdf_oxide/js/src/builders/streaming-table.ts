/**
 * Streaming-table adapter backed by the native row-at-a-time FFI
 * (`pdf_page_builder_streaming_table_begin_v2` / `_push_row` / `_finish`).
 *
 * @example
 * ```typescript
 * const t = page.streamingTable({
 *   columns: [
 *     { header: 'SKU',  width: 72 },
 *     { header: 'Item', width: 200 },
 *     { header: 'Qty',  width: 48, align: Align.Right },
 *   ],
 *   repeatHeader: true,
 *   mode: { kind: 'sample', sampleRows: 30 },
 * });
 * for await (const row of readRowsFromDb()) {
 *   t.pushRow([row.sku, row.item, String(row.qty)]);
 * }
 * await t.finish();
 * ```
 */

import type { Column, SpanCell, StreamingTableConfig } from '../types/common.js';
import type { PageBuilder } from './document-builder.js';

export class StreamingTable {
  private _page: PageBuilder;
  private _columns: Column[];
  private _opened = false;
  private _finished = false;
  private _rowCount = 0;

  /** @internal — constructed via `PageBuilder.streamingTable(...)`. */
  constructor(page: PageBuilder, config: StreamingTableConfig) {
    if (!config || !Array.isArray(config.columns) || config.columns.length === 0) {
      throw new Error('StreamingTable requires at least one column');
    }
    this._page = page;
    this._columns = config.columns;

    const headers = config.columns.map((c) => c.header ?? '');
    const widths = config.columns.map((c) => c.width);
    const aligns = config.columns.map((c) => (c.align ?? 0) as number);
    const repeat = config.repeatHeader !== false;
    const maxRowspan = config.maxRowspan != null && config.maxRowspan >= 2 ? config.maxRowspan : 1;

    this._page._streamingTableBeginV2(headers, widths, aligns, repeat, config.mode, maxRowspan);
    this._opened = true;

    const batchSize = config.batchSize != null && config.batchSize > 0 ? config.batchSize : 256;
    this._page._streamingTableSetBatchSize(batchSize);
  }

  /**
   * Number of rows pushed since the last batch boundary.
   * Backed by the Rust FFI layer.
   */
  get pendingRowCount(): number {
    return this._page._streamingTablePendingRowCount();
  }

  /** Number of complete batches recorded by the native layer so far. */
  get batchCount(): number {
    return this._page._streamingTableBatchCount();
  }

  /**
   * Explicitly mark a batch boundary in the native layer.
   * Also triggered automatically when the configured batch size is reached.
   */
  flush(): this {
    if (this._finished) throw new Error('StreamingTable already finished');
    this._page._streamingTableFlush();
    return this;
  }

  /** Total rows pushed so far (monotonically increasing). */
  get rowCount(): number {
    return this._rowCount;
  }

  /** Push a single row (all rowspan=1). Throws if `cells.length !== columns.length`. */
  pushRow(cells: Array<string | null | undefined>): this {
    if (this._finished) {
      throw new Error('StreamingTable already finished');
    }
    if (cells.length !== this._columns.length) {
      throw new Error(
        `row width ${cells.length} does not match column count ${this._columns.length}`
      );
    }
    this._page._streamingTablePushRow(cells.map((c) => (c == null ? null : String(c))));
    this._rowCount++;
    return this;
  }

  /**
   * Push a single row with per-cell rowspan values. Each element is either
   * a `SpanCell` (`{ text, rowspan }`) or a plain string (rowspan=1).
   * Requires `maxRowspan ≥ 2` in the `StreamingTableConfig`.
   */
  pushRowSpan(cells: Array<SpanCell | string | null | undefined>): this {
    if (this._finished) {
      throw new Error('StreamingTable already finished');
    }
    if (cells.length !== this._columns.length) {
      throw new Error(
        `row width ${cells.length} does not match column count ${this._columns.length}`
      );
    }
    const normalized: Array<[string | null, number]> = cells.map((c) => {
      if (c == null) return [null, 1];
      if (typeof c === 'string') return [c, 1];
      return [c.text, c.rowspan];
    });
    this._page._streamingTablePushRowV2(normalized);
    this._rowCount++;
    return this;
  }

  /**
   * Convenience: consume a sync or async iterable and push each row.
   */
  async pushAll(
    rows:
      | Iterable<Array<string | null | undefined>>
      | AsyncIterable<Array<string | null | undefined>>
  ): Promise<this> {
    if (this._finished) {
      throw new Error('StreamingTable already finished');
    }
    const anyRows = rows as Iterable<Array<string | null | undefined>> &
      Partial<AsyncIterable<Array<string | null | undefined>>>;
    if (typeof anyRows[Symbol.asyncIterator] === 'function') {
      for await (const row of rows as AsyncIterable<Array<string | null | undefined>>) {
        this.pushRow(row);
      }
    } else {
      for (const row of rows as Iterable<Array<string | null | undefined>>) {
        this.pushRow(row);
      }
    }
    return this;
  }

  /**
   * Close the streaming table and return the parent PageBuilder for chaining.
   */
  async finish(): Promise<PageBuilder> {
    if (this._finished) {
      throw new Error('StreamingTable already finished');
    }
    this._finished = true;
    if (this._opened) {
      this._page._streamingTableFinish();
    }
    return this._page;
  }

  /** Number of the columns this table was opened with. */
  get columnCount(): number {
    return this._columns.length;
  }
}
