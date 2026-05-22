/* @ts-self-types="./pdf_oxide.d.ts" */

/**
 * Horizontal-alignment enum shared by `textInRect`, buffered `table`, and
 * `streamingTable`. Maps 1:1 onto [`crate::writer::TextAlign`] /
 * [`crate::writer::CellAlign`]. Exported to JS as `Align` via `js_name`.
 * @enum {0 | 1 | 2}
 */
export const Align = Object.freeze({
    /**
     * Align to the left edge.
     */
    Left: 0, "0": "Left",
    /**
     * Center horizontally.
     */
    Center: 1, "1": "Center",
    /**
     * Align to the right edge.
     */
    Right: 2, "2": "Right",
});

/**
 * Style configuration for header/footer text.
 */
export class ArtifactStyle {
    static __wrap(ptr) {
        const obj = Object.create(ArtifactStyle.prototype);
        obj.__wbg_ptr = ptr;
        ArtifactStyleFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ArtifactStyleFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_artifactstyle_free(ptr, 0);
    }
    /**
     * Set bold font for the artifact.
     * @returns {ArtifactStyle}
     */
    bold() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.wasmartifactstyle_bold(ptr);
        return ArtifactStyle.__wrap(ret);
    }
    /**
     * Set color for the artifact.
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @returns {ArtifactStyle}
     */
    color(r, g, b) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.wasmartifactstyle_color(ptr, r, g, b);
        return ArtifactStyle.__wrap(ret);
    }
    /**
     * Set font for the artifact.
     * @param {string} name
     * @param {number} size
     * @returns {ArtifactStyle}
     */
    font(name, size) {
        const ptr = this.__destroy_into_raw();
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmartifactstyle_font(ptr, ptr0, len0, size);
        return ArtifactStyle.__wrap(ret);
    }
    /**
     * Create a new artifact style.
     */
    constructor() {
        const ret = wasm.wasmartifactstyle_new();
        this.__wbg_ptr = ret;
        ArtifactStyleFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) ArtifactStyle.prototype[Symbol.dispose] = ArtifactStyle.prototype.free;

/**
 * Chroma subsampling format
 * @enum {0 | 1 | 2 | 3}
 */
export const ChromaSampling = Object.freeze({
    /**
     * Both vertically and horizontally subsampled.
     */
    Cs420: 0, "0": "Cs420",
    /**
     * Horizontally subsampled.
     */
    Cs422: 1, "1": "Cs422",
    /**
     * Not subsampled.
     */
    Cs444: 2, "2": "Cs444",
    /**
     * Monochrome.
     */
    Cs400: 3, "3": "Cs400",
});

/**
 * WASM handle to a streaming-table building session. Created by
 * `FluentPageBuilder.streamingTable()`; rows are pushed via `pushRow`,
 * and the session is sealed with `finish()`.
 *
 * Single-use: `finish()` twice throws, and `pushRow` after `finish()`
 * throws. The rows are buffered and replayed against the real Rust
 * `StreamingTable` at `WasmFluentPageBuilder.done()` commit time —
 * preserving the FluentPageBuilder borrow-lifetime invariant that can't
 * cross the wasm-bindgen boundary.
 */
export class StreamingTable {
    static __wrap(ptr) {
        const obj = Object.create(StreamingTable.prototype);
        obj.__wbg_ptr = ptr;
        StreamingTableFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        StreamingTableFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_streamingtable_free(ptr, 0);
    }
    /**
     * Number of fully-completed batches waiting for finish().
     * @returns {number}
     */
    batchCount() {
        const ret = wasm.streamingtable_batchCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Number of columns configured on this streaming table.
     * @returns {number}
     */
    columnCount() {
        const ret = wasm.streamingtable_columnCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Seal the streaming table — the buffered rows are flushed onto the
     * parent page's op queue, to be replayed against the real Rust
     * `StreamingTable` at `done()` commit time. Calling `finish()` twice
     * throws.
     */
    finish() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.streamingtable_finish(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Explicitly flush the current batch to `completed_batches`.
     * Called automatically when `batch_size` rows accumulate.
     */
    flush() {
        wasm.streamingtable_flush(this.__wbg_ptr);
    }
    /**
     * Number of rows in the current (not-yet-flushed) batch.
     * @returns {number}
     */
    pendingRowCount() {
        const ret = wasm.streamingtable_pendingRowCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Push one row as an array of cell strings (all rowspan=1). Returns an
     * error if the table has already been finished or if the row's cell count
     * does not match the column count. Auto-flushes the batch when full.
     * @param {string[]} cells
     */
    pushRow(cells) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayJsValueToWasm0(cells, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.streamingtable_pushRow(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Push one row with per-cell rowspan values. `cells` is a JS array of
     * `[text, rowspan]` two-element arrays. `rowspan == 1` is a normal cell.
     * Auto-flushes the batch when full.
     * @param {any} cells
     */
    pushRowSpan(cells) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.streamingtable_pushRowSpan(retptr, this.__wbg_ptr, addHeapObject(cells));
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) StreamingTable.prototype[Symbol.dispose] = StreamingTable.prototype.free;

/**
 * A header or footer artifact definition.
 */
export class WasmArtifact {
    static __wrap(ptr) {
        const obj = Object.create(WasmArtifact.prototype);
        obj.__wbg_ptr = ptr;
        WasmArtifactFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmArtifactFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmartifact_free(ptr, 0);
    }
    /**
     * Create a center-aligned artifact.
     * @param {string} text
     * @returns {WasmArtifact}
     */
    static center(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmartifact_center(ptr0, len0);
        return WasmArtifact.__wrap(ret);
    }
    /**
     * Create a left-aligned artifact.
     * @param {string} text
     * @returns {WasmArtifact}
     */
    static left(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmartifact_left(ptr0, len0);
        return WasmArtifact.__wrap(ret);
    }
    /**
     * Create a new artifact.
     */
    constructor() {
        const ret = wasm.wasmartifact_new();
        this.__wbg_ptr = ret;
        WasmArtifactFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Create a right-aligned artifact.
     * @param {string} text
     * @returns {WasmArtifact}
     */
    static right(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmartifact_right(ptr0, len0);
        return WasmArtifact.__wrap(ret);
    }
    /**
     * Set vertical offset for the artifact.
     * @param {number} offset
     * @returns {WasmArtifact}
     */
    withOffset(offset) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.wasmartifact_withOffset(ptr, offset);
        return WasmArtifact.__wrap(ret);
    }
    /**
     * Set style for the artifact.
     * @param {ArtifactStyle} style
     * @returns {WasmArtifact}
     */
    withStyle(style) {
        const ptr = this.__destroy_into_raw();
        _assertClass(style, ArtifactStyle);
        const ret = wasm.wasmartifact_withStyle(ptr, style.__wbg_ptr);
        return WasmArtifact.__wrap(ret);
    }
}
if (Symbol.dispose) WasmArtifact.prototype[Symbol.dispose] = WasmArtifact.prototype.free;

/**
 * X.509 certificate parsed from a raw DER blob. Mirrors the C#,
 * Node, Python, and Go `Certificate` surfaces — `subject` / `issuer`
 * / `serial` / `validity` / `isValid` getters only.
 */
export class WasmCertificate {
    static __wrap(ptr) {
        const obj = Object.create(WasmCertificate.prototype);
        obj.__wbg_ptr = ptr;
        WasmCertificateFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmCertificateFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmcertificate_free(ptr, 0);
    }
    /**
     * Whether the certificate is currently within its validity
     * window. Does NOT verify chain, trust-root, or revocation.
     * @returns {boolean}
     */
    get isValid() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmcertificate_isValid(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 !== 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Issuer distinguished name.
     * @returns {string}
     */
    get issuer() {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmcertificate_issuer(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Load a certificate from a DER-encoded X.509 blob. Throws if
     * the DER doesn't parse.
     * @param {Uint8Array} data
     * @returns {WasmCertificate}
     */
    static load(data) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmcertificate_load(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmCertificate.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Load a signer certificate + private key from PEM strings.
     * `certPem` must begin `-----BEGIN CERTIFICATE-----`.
     * `keyPem` must begin `-----BEGIN PRIVATE KEY-----` or `-----BEGIN RSA PRIVATE KEY-----`.
     * @param {string} cert_pem
     * @param {string} key_pem
     * @returns {WasmCertificate}
     */
    static loadPem(cert_pem, key_pem) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(cert_pem, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(key_pem, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            wasm.wasmcertificate_loadPem(retptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmCertificate.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Load a signer certificate + private key from a PKCS#12 (.p12/.pfx) blob.
     * `password` is the passphrase protecting the key bag.
     * @param {Uint8Array} data
     * @param {string} password
     * @returns {WasmCertificate}
     */
    static loadPkcs12(data, password) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(password, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            wasm.wasmcertificate_loadPkcs12(retptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmCertificate.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Serial number as a hex string (no `0x` prefix).
     * @returns {string}
     */
    get serial() {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmcertificate_serial(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Subject distinguished name.
     * @returns {string}
     */
    get subject() {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmcertificate_subject(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Validity window as `[notBefore, notAfter]` Unix epoch seconds.
     * JavaScript: `new Date(notBefore * 1000)` for a Date.
     * @returns {BigInt64Array}
     */
    get validity() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmcertificate_validity(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayI64FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 8, 8);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmCertificate.prototype[Symbol.dispose] = WasmCertificate.prototype.free;

/**
 * WASM wrapper for [`crate::writer::DocumentBuilder`]. Fluent API for
 * programmatic multi-page PDF construction with embedded fonts and
 * annotations.
 *
 * The terminal methods (`build`, `toBytesEncrypted`) **consume** the
 * builder; subsequent calls throw `Error: DocumentBuilder already
 * consumed`.
 */
export class WasmDocumentBuilder {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmDocumentBuilderFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmdocumentbuilder_free(ptr, 0);
    }
    /**
     * Start a new A4 page. Returns a `FluentPageBuilder` that must be
     * committed with `.done()` before calling another page method or a
     * terminal (`build`, etc.).
     * @returns {WasmFluentPageBuilder}
     */
    a4Page() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmdocumentbuilder_a4Page(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmFluentPageBuilder.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set document author.
     * @param {string} author
     */
    author(author) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(author, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmdocumentbuilder_author(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Build the PDF and return it as a `Uint8Array`. **Consumes** the
     * builder.
     * @returns {Uint8Array}
     */
    build() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmdocumentbuilder_build(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Commit a completed `FluentPageBuilder` back to this builder.
     * Takes the place of the Rust `page.done()` re-parenting.
     *
     * JS users typically don't call this directly — the ergonomic
     * pattern is `builder.a4Page();` for each page, then
     * `builder.commitPage(page)` once ops are queued. For more fluent
     * code, see the `FluentPageBuilder.done(builder)` helper which
     * delegates to this method.
     * @param {WasmFluentPageBuilder} page
     */
    commitPage(page) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            _assertClass(page, WasmFluentPageBuilder);
            wasm.wasmdocumentbuilder_commitPage(retptr, this.__wbg_ptr, page.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the creator application name recorded in the PDF.
     * @param {string} creator
     */
    creator(creator) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(creator, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmdocumentbuilder_creator(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set document keywords (comma-separated per PDF convention).
     * @param {string} keywords
     */
    keywords(keywords) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(keywords, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmdocumentbuilder_keywords(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the document's natural language tag (e.g. `"en-US"`).
     *
     * Emitted as `/Lang` in the catalog when `taggedPdfUa1()` is set.
     * @param {string} lang
     */
    language(lang) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(lang, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmdocumentbuilder_language(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Start a new US Letter page.
     * @returns {WasmFluentPageBuilder}
     */
    letterPage() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmdocumentbuilder_letterPage(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmFluentPageBuilder.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create a new empty document builder. Equivalent to the Rust
     * [`crate::writer::DocumentBuilder::new`] — every other method
     * chains off the instance returned here.
     */
    constructor() {
        const ret = wasm.wasmdocumentbuilder_new();
        this.__wbg_ptr = ret;
        WasmDocumentBuilderFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Run a JavaScript script when the document is opened (`/OpenAction`).
     * @param {string} script
     */
    onOpen(script) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(script, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmdocumentbuilder_onOpen(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Start a new page with custom dimensions in PDF points
     * (72 pt = 1 inch).
     * @param {number} width
     * @param {number} height
     * @returns {WasmFluentPageBuilder}
     */
    page(width, height) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmdocumentbuilder_page(retptr, this.__wbg_ptr, width, height);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmFluentPageBuilder.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Register a TTF / OTF font the pages can reference by name.
     * **Consumes** `font` — reusing the `WasmEmbeddedFont` throws.
     * @param {string} name
     * @param {WasmEmbeddedFont} font
     */
    registerEmbeddedFont(name, font) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            _assertClass(font, WasmEmbeddedFont);
            wasm.wasmdocumentbuilder_registerEmbeddedFont(retptr, this.__wbg_ptr, ptr0, len0, font.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Add a role-map entry: custom structure type → standard PDF structure type.
     *
     * Emitted in `/RoleMap` inside the StructTreeRoot when `taggedPdfUa1()`
     * is set. Multiple calls accumulate entries.
     * @param {string} custom
     * @param {string} standard
     */
    roleMap(custom, standard) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(custom, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(standard, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            wasm.wasmdocumentbuilder_roleMap(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set document subject.
     * @param {string} subject
     */
    subject(subject) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(subject, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmdocumentbuilder_subject(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Enable PDF/UA-1 tagged PDF mode.
     *
     * When enabled, `build()` emits `/MarkInfo`, `/StructTreeRoot`, `/Lang`,
     * and `/ViewerPreferences` in the catalog. Opt-in — no effect unless called.
     */
    taggedPdfUa1() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmdocumentbuilder_taggedPdfUa1(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set document title.
     * @param {string} title
     */
    title(title) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(title, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmdocumentbuilder_title(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Build the PDF with AES-256 encryption and return it as a
     * `Uint8Array`. Granted permissions default to all. **Consumes**
     * the builder.
     * @param {string} user_password
     * @param {string} owner_password
     * @returns {Uint8Array}
     */
    toBytesEncrypted(user_password, owner_password) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(user_password, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(owner_password, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            wasm.wasmdocumentbuilder_toBytesEncrypted(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v3 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v3;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmDocumentBuilder.prototype[Symbol.dispose] = WasmDocumentBuilder.prototype.free;

/**
 * Embedded TTF/OTF font usable by `WasmDocumentBuilder`. Single-use: once
 * passed to `registerEmbeddedFont`, the underlying Rust `EmbeddedFont` is
 * moved into the builder and this handle becomes empty.
 */
export class WasmEmbeddedFont {
    static __wrap(ptr) {
        const obj = Object.create(WasmEmbeddedFont.prototype);
        obj.__wbg_ptr = ptr;
        WasmEmbeddedFontFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmEmbeddedFontFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmembeddedfont_free(ptr, 0);
    }
    /**
     * Load an embedded font from raw TTF/OTF bytes. Pass `name` to
     * override the PostScript name baked into the font file.
     * @param {Uint8Array} data
     * @param {string | null} [name]
     * @returns {WasmEmbeddedFont}
     */
    static fromBytes(data, name) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            var ptr1 = isLikeNone(name) ? 0 : passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            wasm.wasmembeddedfont_fromBytes(retptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmEmbeddedFont.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * The font's PostScript name (or the override). Empty once consumed.
     * @returns {string}
     */
    get name() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmembeddedfont_name(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) WasmEmbeddedFont.prototype[Symbol.dispose] = WasmEmbeddedFont.prototype.free;

/**
 * Per-page fluent builder. Buffers operations until `done(builder)` is
 * called, which commits them to the parent `WasmDocumentBuilder`. Each
 * instance is single-use — `done()` twice throws.
 */
export class WasmFluentPageBuilder {
    static __wrap(ptr) {
        const obj = Object.create(WasmFluentPageBuilder.prototype);
        obj.__wbg_ptr = ptr;
        WasmFluentPageBuilderFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmFluentPageBuilderFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmfluentpagebuilder_free(ptr, 0);
    }
    /**
     * @param {number} x
     * @param {number} y
     */
    at(x, y) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_at(retptr, this.__wbg_ptr, x, y);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Place a 1-D barcode image at `(x, y, w, h)` on the page.
     * `barcodeType`: 0=Code128 1=Code39 2=EAN13 3=EAN8 4=UPCA 5=ITF
     * 6=Code93 7=Codabar.
     * @param {number} barcode_type
     * @param {string} data
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     */
    barcode1d(barcode_type, data, x, y, w, h) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(data, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_barcode1d(retptr, this.__wbg_ptr, barcode_type, ptr0, len0, x, y, w, h);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Place a QR-code image at `(x, y, size, size)` on the page.
     * @param {string} data
     * @param {number} x
     * @param {number} y
     * @param {number} size
     */
    barcodeQr(data, x, y, size) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(data, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_barcodeQr(retptr, this.__wbg_ptr, ptr0, len0, x, y, size);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} name
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {boolean} checked
     */
    checkbox(name, x, y, w, h, checked) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_checkbox(retptr, this.__wbg_ptr, ptr0, len0, x, y, w, h, checked);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Lay out `text` as balanced multi-column flow (`columnCount` columns,
     * `gapPt` points between columns). Paragraphs in `text` are separated by `"\n\n"`.
     * @param {number} column_count
     * @param {number} gap_pt
     * @param {string} text
     */
    columns(column_count, gap_pt, text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_columns(retptr, this.__wbg_ptr, column_count, gap_pt, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Add a dropdown combo-box.
     * @param {string} name
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {string[]} options
     * @param {string | null} [selected]
     */
    comboBox(name, x, y, w, h, options, selected) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArrayJsValueToWasm0(options, wasm.__wbindgen_export);
            const len1 = WASM_VECTOR_LEN;
            var ptr2 = isLikeNone(selected) ? 0 : passStringToWasm0(selected, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len2 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_comboBox(retptr, this.__wbg_ptr, ptr0, len0, x, y, w, h, ptr1, len1, ptr2, len2);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Convenience: commit this page's buffered ops to `builder`. Same
     * as `builder.commitPage(this)` but lets JS users keep the
     * chain-like flow:
     *
     * ```javascript
     * const page = builder.a4Page();
     * page.at(72, 720); page.text("Hi");
     * page.done(builder);
     * ```
     * @param {WasmDocumentBuilder} builder
     */
    done(builder) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            _assertClass(builder, WasmDocumentBuilder);
            wasm.wasmfluentpagebuilder_done(retptr, this.__wbg_ptr, builder.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} script
     */
    fieldCalculate(script) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(script, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_fieldCalculate(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} script
     */
    fieldFormat(script) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(script, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_fieldFormat(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} script
     */
    fieldKeystroke(script) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(script, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_fieldKeystroke(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} script
     */
    fieldValidate(script) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(script, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_fieldValidate(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Draw a filled rectangle. RGB channels in 0.0-1.0.
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    filledRect(x, y, w, h, r, g, b) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_filledRect(retptr, this.__wbg_ptr, x, y, w, h, r, g, b);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} name
     * @param {number} size
     */
    font(name, size) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_font(retptr, this.__wbg_ptr, ptr0, len0, size);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Add a footnote: inline `refMark` at the cursor and `noteText` body
     * near the page bottom with a separator artifact line.
     * @param {string} ref_mark
     * @param {string} note_text
     */
    footnote(ref_mark, note_text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(ref_mark, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(note_text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_footnote(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {string} text
     */
    freeText(x, y, w, h, text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_freeText(retptr, this.__wbg_ptr, x, y, w, h, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} level
     * @param {string} text
     */
    heading(level, text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_heading(retptr, this.__wbg_ptr, level, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    highlight(r, g, b) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_highlight(retptr, this.__wbg_ptr, r, g, b);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    horizontalRule() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_horizontalRule(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Embed a decorative image (JPEG/PNG/WebP bytes) as an /Artifact (no alt text).
     * @param {Uint8Array} bytes
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     */
    imageArtifact(bytes, x, y, w, h) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_imageArtifact(retptr, this.__wbg_ptr, ptr0, len0, x, y, w, h);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Embed an image (JPEG/PNG/WebP bytes) with an accessibility alt text.
     * @param {Uint8Array} bytes
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {string} alt_text
     */
    imageWithAlt(bytes, x, y, w, h, alt_text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(alt_text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_imageWithAlt(retptr, this.__wbg_ptr, ptr0, len0, x, y, w, h, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Emit `text` inline (advances cursorX only, not cursorY).
     * @param {string} text
     */
    inline(text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_inline(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Inline bold run.
     * @param {string} text
     */
    inlineBold(text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_inlineBold(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Inline colored run (RGB 0.0–1.0).
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {string} text
     */
    inlineColor(r, g, b, text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_inlineColor(retptr, this.__wbg_ptr, r, g, b, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Inline italic run.
     * @param {string} text
     */
    inlineItalic(text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_inlineItalic(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Draw a line from (x1, y1) to (x2, y2) with 1pt black stroke.
     * @param {number} x1
     * @param {number} y1
     * @param {number} x2
     * @param {number} y2
     */
    line(x1, y1, x2, y2) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_line(retptr, this.__wbg_ptr, x1, y1, x2, y2);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} script
     */
    linkJavascript(script) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(script, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_linkJavascript(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} destination
     */
    linkNamed(destination) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(destination, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_linkNamed(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} page
     */
    linkPage(page) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_linkPage(retptr, this.__wbg_ptr, page);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} url
     */
    linkUrl(url) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(url, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_linkUrl(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Measure the rendered width of `text` in the builder's current font
     * and size, in PDF points. Pure query — does not mutate state.
     *
     * Thin JS view over [`crate::writer::FluentPageBuilder::measure`]. The
     * WASM class tracks the current font/size independently of the
     * buffered ops so this query is served without a live builder.
     * @param {string} text
     * @returns {number}
     */
    measure(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmfluentpagebuilder_measure(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Finish the current page and start a new one with the same page
     * size. Cursor resets to the top-left margin (72, height-72). The
     * builder's font carries over.
     */
    newPageSameSize() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_newPageSameSize(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Advance cursorY by one line-height and reset cursorX to 72 pt.
     */
    newline() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_newline(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} script
     */
    onClose(script) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(script, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_onClose(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} script
     */
    onOpen(script) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(script, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_onOpen(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} text
     */
    paragraph(text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_paragraph(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Add a clickable push button with a visible caption.
     * @param {string} name
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {string} caption
     */
    pushButton(name, x, y, w, h, caption) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(caption, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_pushButton(retptr, this.__wbg_ptr, ptr0, len0, x, y, w, h, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Add a radio-button group. `values`, `xs`, `ys`, `ws`, `hs` are
     * parallel arrays of length N describing each option's export
     * value and rectangle. `selected` picks the initial value.
     * @param {string} name
     * @param {string[]} values
     * @param {Float32Array} xs
     * @param {Float32Array} ys
     * @param {Float32Array} ws
     * @param {Float32Array} hs
     * @param {string | null} [selected]
     */
    radioGroup(name, values, xs, ys, ws, hs, selected) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArrayJsValueToWasm0(values, wasm.__wbindgen_export);
            const len1 = WASM_VECTOR_LEN;
            const ptr2 = passArrayF32ToWasm0(xs, wasm.__wbindgen_export);
            const len2 = WASM_VECTOR_LEN;
            const ptr3 = passArrayF32ToWasm0(ys, wasm.__wbindgen_export);
            const len3 = WASM_VECTOR_LEN;
            const ptr4 = passArrayF32ToWasm0(ws, wasm.__wbindgen_export);
            const len4 = WASM_VECTOR_LEN;
            const ptr5 = passArrayF32ToWasm0(hs, wasm.__wbindgen_export);
            const len5 = WASM_VECTOR_LEN;
            var ptr6 = isLikeNone(selected) ? 0 : passStringToWasm0(selected, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len6 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_radioGroup(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Draw a stroked rectangle outline (1pt black).
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     */
    rect(x, y, w, h) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_rect(retptr, this.__wbg_ptr, x, y, w, h);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Points remaining on the current page below the cursor (down to the
     * 72 pt bottom margin). Mirrors
     * [`crate::writer::FluentPageBuilder::remaining_space`] using the WASM
     * class's independently-tracked cursor — accurate after `at`, `text`,
     * `space`, `newPageSameSize`, `textInRect` and the table helpers.
     * @returns {number}
     */
    remainingSpace() {
        const ret = wasm.wasmfluentpagebuilder_remainingSpace(this.__wbg_ptr);
        return ret;
    }
    /**
     * Add an unsigned signature placeholder field at the given bounds.
     * @param {string} name
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     */
    signatureField(name, x, y, w, h) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_signatureField(retptr, this.__wbg_ptr, ptr0, len0, x, y, w, h);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} points
     */
    space(points) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_space(retptr, this.__wbg_ptr, points);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    squiggly(r, g, b) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_squiggly(retptr, this.__wbg_ptr, r, g, b);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} name
     */
    stamp(name) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_stamp(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} text
     */
    stickyNote(text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_stickyNote(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {string} text
     */
    stickyNoteAt(x, y, text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_stickyNoteAt(retptr, this.__wbg_ptr, x, y, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Open a streaming table. Returns a `StreamingTable` handle the caller
     * pushes rows into; call `finish()` when done. The streamed rows are
     * buffered per-table and replayed against the real Rust
     * `StreamingTable` at `done()` commit time — avoiding the
     * FluentPageBuilder-lifetime problem that otherwise can't cross the
     * wasm-bindgen boundary.
     * @param {any} spec
     * @returns {StreamingTable}
     */
    streamingTable(spec) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_streamingTable(retptr, this.__wbg_ptr, addHeapObject(spec));
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return StreamingTable.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    strikeout(r, g, b) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_strikeout(retptr, this.__wbg_ptr, r, g, b);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Draw a straight line with explicit stroke width and RGB colour.
     * @param {number} x1
     * @param {number} y1
     * @param {number} x2
     * @param {number} y2
     * @param {number} width
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    strokeLine(x1, y1, x2, y2, width, r, g, b) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_strokeLine(retptr, this.__wbg_ptr, x1, y1, x2, y2, width, r, g, b);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Draw a dashed line. `dash` is alternating on/off lengths in points; `phase` is the starting offset.
     * @param {number} x1
     * @param {number} y1
     * @param {number} x2
     * @param {number} y2
     * @param {number} width
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {Float32Array} dash
     * @param {number} phase
     */
    strokeLineDashed(x1, y1, x2, y2, width, r, g, b, dash, phase) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(dash, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_strokeLineDashed(retptr, this.__wbg_ptr, x1, y1, x2, y2, width, r, g, b, ptr0, len0, phase);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Draw a stroked rectangle with explicit stroke width and RGB colour.
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {number} width
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    strokeRect(x, y, w, h, width, r, g, b) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_strokeRect(retptr, this.__wbg_ptr, x, y, w, h, width, r, g, b);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Draw a dashed rectangle border. `dash` is alternating on/off lengths in points; `phase` is the starting offset.
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {number} width
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {Float32Array} dash
     * @param {number} phase
     */
    strokeRectDashed(x, y, w, h, width, r, g, b, dash, phase) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(dash, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_strokeRectDashed(retptr, this.__wbg_ptr, x, y, w, h, width, r, g, b, ptr0, len0, phase);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Render a buffered table from a JS object:
     *
     * ```javascript
     * page.table({
     *   columns: [
     *     { header: "SKU", width: 100, align: Align.Left },
     *     { header: "Qty", width: 60,  align: Align.Right },
     *   ],
     *   rows: [["A-1","12"], ["B-2","3"]],
     *   hasHeader: true,
     * });
     * ```
     *
     * Uses `serde-wasm-bindgen` for deserialisation. Replays against the
     * Rust `Table` builder at `done()` commit time.
     * @param {any} spec
     */
    table(spec) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_table(retptr, this.__wbg_ptr, addHeapObject(spec));
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} text
     */
    text(text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_text(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} name
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {string | null} [default_value]
     */
    textField(name, x, y, w, h, default_value) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            var ptr1 = isLikeNone(default_value) ? 0 : passStringToWasm0(default_value, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_textField(retptr, this.__wbg_ptr, ptr0, len0, x, y, w, h, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Place wrapped text inside a rectangle with horizontal alignment.
     * `align` is the `Align` enum (0 = Left, 1 = Center, 2 = Right) — also
     * accepts a raw integer for JS callers that pre-date the enum import.
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {string} text
     * @param {number} align
     */
    textInRect(x, y, w, h, text, align) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_textInRect(retptr, this.__wbg_ptr, x, y, w, h, ptr0, len0, align);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    underline(r, g, b) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_underline(retptr, this.__wbg_ptr, r, g, b);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {string} text
     */
    watermark(text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmfluentpagebuilder_watermark(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    watermarkConfidential() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_watermarkConfidential(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    watermarkDraft() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmfluentpagebuilder_watermarkDraft(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmFluentPageBuilder.prototype[Symbol.dispose] = WasmFluentPageBuilder.prototype.free;

/**
 * A footer definition.
 */
export class WasmFooter {
    static __wrap(ptr) {
        const obj = Object.create(WasmFooter.prototype);
        obj.__wbg_ptr = ptr;
        WasmFooterFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmFooterFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmfooter_free(ptr, 0);
    }
    /**
     * Create a center-aligned footer.
     * @param {string} text
     * @returns {WasmFooter}
     */
    static center(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmfooter_center(ptr0, len0);
        return WasmFooter.__wrap(ret);
    }
    /**
     * Create a left-aligned footer.
     * @param {string} text
     * @returns {WasmFooter}
     */
    static left(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmfooter_left(ptr0, len0);
        return WasmFooter.__wrap(ret);
    }
    /**
     * Create a new empty footer.
     */
    constructor() {
        const ret = wasm.wasmfooter_new();
        this.__wbg_ptr = ret;
        WasmFooterFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Create a right-aligned footer.
     * @param {string} text
     * @returns {WasmFooter}
     */
    static right(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmfooter_right(ptr0, len0);
        return WasmFooter.__wrap(ret);
    }
}
if (Symbol.dispose) WasmFooter.prototype[Symbol.dispose] = WasmFooter.prototype.free;

/**
 * A header definition.
 */
export class WasmHeader {
    static __wrap(ptr) {
        const obj = Object.create(WasmHeader.prototype);
        obj.__wbg_ptr = ptr;
        WasmHeaderFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmHeaderFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmheader_free(ptr, 0);
    }
    /**
     * Create a center-aligned header.
     * @param {string} text
     * @returns {WasmHeader}
     */
    static center(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmheader_center(ptr0, len0);
        return WasmHeader.__wrap(ret);
    }
    /**
     * Create a left-aligned header.
     * @param {string} text
     * @returns {WasmHeader}
     */
    static left(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmheader_left(ptr0, len0);
        return WasmHeader.__wrap(ret);
    }
    /**
     * Create a new empty header.
     */
    constructor() {
        const ret = wasm.wasmheader_new();
        this.__wbg_ptr = ret;
        WasmHeaderFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Create a right-aligned header.
     * @param {string} text
     * @returns {WasmHeader}
     */
    static right(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmheader_right(ptr0, len0);
        return WasmHeader.__wrap(ret);
    }
}
if (Symbol.dispose) WasmHeader.prototype[Symbol.dispose] = WasmHeader.prototype.free;

/**
 * OCR configuration for WebAssembly.
 */
export class WasmOcrConfig {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmOcrConfigFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmocrconfig_free(ptr, 0);
    }
    /**
     * Create a new OCR configuration.
     */
    constructor() {
        const ret = wasm.wasmocrconfig_new();
        this.__wbg_ptr = ret;
        WasmOcrConfigFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmOcrConfig.prototype[Symbol.dispose] = WasmOcrConfig.prototype.free;

/**
 * OCR engine for WebAssembly.
 */
export class WasmOcrEngine {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmOcrEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmocrengine_free(ptr, 0);
    }
    /**
     * Create a new OCR engine.
     * @param {string} _det_model_path
     * @param {string} _rec_model_path
     * @param {string} _dict_path
     * @param {WasmOcrConfig | null} [_config]
     */
    constructor(_det_model_path, _rec_model_path, _dict_path, _config) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(_det_model_path, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(_rec_model_path, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            const ptr2 = passStringToWasm0(_dict_path, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len2 = WASM_VECTOR_LEN;
            let ptr3 = 0;
            if (!isLikeNone(_config)) {
                _assertClass(_config, WasmOcrConfig);
                ptr3 = _config.__destroy_into_raw();
            }
            wasm.wasmocrengine_new(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0;
            WasmOcrEngineFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmOcrEngine.prototype[Symbol.dispose] = WasmOcrEngine.prototype.free;

/**
 * A complete page template with header and footer.
 */
export class WasmPageTemplate {
    static __wrap(ptr) {
        const obj = Object.create(WasmPageTemplate.prototype);
        obj.__wbg_ptr = ptr;
        WasmPageTemplateFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmPageTemplateFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmpagetemplate_free(ptr, 0);
    }
    /**
     * Set footer artifact.
     * @param {WasmArtifact} footer
     * @returns {WasmPageTemplate}
     */
    footer(footer) {
        const ptr = this.__destroy_into_raw();
        _assertClass(footer, WasmArtifact);
        const ret = wasm.wasmpagetemplate_footer(ptr, footer.__wbg_ptr);
        return WasmPageTemplate.__wrap(ret);
    }
    /**
     * Set header artifact.
     * @param {WasmArtifact} header
     * @returns {WasmPageTemplate}
     */
    header(header) {
        const ptr = this.__destroy_into_raw();
        _assertClass(header, WasmArtifact);
        const ret = wasm.wasmpagetemplate_header(ptr, header.__wbg_ptr);
        return WasmPageTemplate.__wrap(ret);
    }
    /**
     * Create a new page template.
     */
    constructor() {
        const ret = wasm.wasmpagetemplate_new();
        this.__wbg_ptr = ret;
        WasmPageTemplateFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Skip rendering template on the first page.
     * @returns {WasmPageTemplate}
     */
    skipFirstPage() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.wasmpagetemplate_skipFirstPage(ptr);
        return WasmPageTemplate.__wrap(ret);
    }
}
if (Symbol.dispose) WasmPageTemplate.prototype[Symbol.dispose] = WasmPageTemplate.prototype.free;

/**
 * Create new PDF documents from Markdown, HTML, or plain text.
 *
 * ```javascript
 * const pdf = WasmPdf.fromMarkdown("# Hello\n\nWorld");
 * const bytes = pdf.toBytes(); // Uint8Array
 * console.log(`PDF size: ${pdf.size} bytes`);
 * ```
 */
export class WasmPdf {
    static __wrap(ptr) {
        const obj = Object.create(WasmPdf.prototype);
        obj.__wbg_ptr = ptr;
        WasmPdfFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmPdfFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmpdf_free(ptr, 0);
    }
    /**
     * Open an existing PDF from bytes for editing.
     *
     * @param data - PDF file contents as Uint8Array
     * @returns WasmPdf for editing
     * @param {Uint8Array} data
     * @returns {WasmPdf}
     */
    static fromBytes(data) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdf_fromBytes(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdf.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create a PDF from HTML content.
     *
     * @param content - HTML string
     * @param title - Optional document title
     * @param author - Optional document author
     * @param {string} content
     * @param {string | null} [title]
     * @param {string | null} [author]
     * @returns {WasmPdf}
     */
    static fromHtml(content, title, author) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(content, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            var ptr1 = isLikeNone(title) ? 0 : passStringToWasm0(title, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            var ptr2 = isLikeNone(author) ? 0 : passStringToWasm0(author, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len2 = WASM_VECTOR_LEN;
            wasm.wasmpdf_fromHtml(retptr, ptr0, len0, ptr1, len1, ptr2, len2);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdf.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Render `html` with `css` applied, embedding `font_bytes` for the
     * body text. The font must cover every codepoint used by `html` or
     * unknown glyphs fall back to `.notdef`. See
     * [`Self::from_html_css_with_fonts`] for a multi-font cascade.
     * @param {string} html
     * @param {string} css
     * @param {Uint8Array} font_bytes
     * @returns {WasmPdf}
     */
    static fromHtmlCss(html, css, font_bytes) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(html, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(css, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            const ptr2 = passArray8ToWasm0(font_bytes, wasm.__wbindgen_export);
            const len2 = WASM_VECTOR_LEN;
            wasm.wasmpdf_fromHtmlCss(retptr, ptr0, len0, ptr1, len1, ptr2, len2);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdf.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Render `html` + `css` with a multi-font cascade. `families` and
     * `fonts` are parallel arrays: `families[i]` names the CSS
     * `font-family` that resolves to `fonts[i]` bytes. The first entry
     * is the default used whenever a CSS `font-family` doesn't match a
     * registered family.
     * @param {string} html
     * @param {string} css
     * @param {string[]} families
     * @param {Uint8Array[]} fonts
     * @returns {WasmPdf}
     */
    static fromHtmlCssWithFonts(html, css, families, fonts) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(html, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(css, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            const ptr2 = passArrayJsValueToWasm0(families, wasm.__wbindgen_export);
            const len2 = WASM_VECTOR_LEN;
            const ptr3 = passArrayJsValueToWasm0(fonts, wasm.__wbindgen_export);
            const len3 = WASM_VECTOR_LEN;
            wasm.wasmpdf_fromHtmlCssWithFonts(retptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdf.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create a PDF from image bytes (PNG, JPEG, etc.).
     *
     * @param data - Image file contents as a Uint8Array
     * @param {Uint8Array} data
     * @returns {WasmPdf}
     */
    static fromImageBytes(data) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdf_fromImageBytes(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdf.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create a PDF from Markdown content.
     *
     * @param content - Markdown string
     * @param title - Optional document title
     * @param author - Optional document author
     * @param {string} content
     * @param {string | null} [title]
     * @param {string | null} [author]
     * @returns {WasmPdf}
     */
    static fromMarkdown(content, title, author) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(content, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            var ptr1 = isLikeNone(title) ? 0 : passStringToWasm0(title, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            var ptr2 = isLikeNone(author) ? 0 : passStringToWasm0(author, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len2 = WASM_VECTOR_LEN;
            wasm.wasmpdf_fromMarkdown(retptr, ptr0, len0, ptr1, len1, ptr2, len2);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdf.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create a PDF from multiple image byte arrays.
     *
     * Each image becomes a separate page. Pass an array of Uint8Arrays.
     *
     * @param images_array - Array of Uint8Arrays, each containing image file bytes (PNG/JPEG)
     * @param {any} images_array
     * @returns {WasmPdf}
     */
    static fromMultipleImageBytes(images_array) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdf_fromMultipleImageBytes(retptr, addHeapObject(images_array));
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdf.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create a PDF from plain text.
     *
     * @param content - Plain text string
     * @param title - Optional document title
     * @param author - Optional document author
     * @param {string} content
     * @param {string | null} [title]
     * @param {string | null} [author]
     * @returns {WasmPdf}
     */
    static fromText(content, title, author) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(content, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            var ptr1 = isLikeNone(title) ? 0 : passStringToWasm0(title, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            var ptr2 = isLikeNone(author) ? 0 : passStringToWasm0(author, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len2 = WASM_VECTOR_LEN;
            wasm.wasmpdf_fromText(retptr, ptr0, len0, ptr1, len1, ptr2, len2);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdf.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Merge multiple PDF byte arrays into a single PDF.
     *
     * @param pdfs - Array of Uint8Array, each containing a PDF
     * @returns WasmPdf containing all pages
     * @param {Uint8Array[]} pdfs
     * @returns {WasmPdf}
     */
    static merge(pdfs) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayJsValueToWasm0(pdfs, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdf_merge(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdf.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get the size of the PDF in bytes.
     * @returns {number}
     */
    get size() {
        const ret = wasm.wasmpdf_size(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the PDF as a Uint8Array.
     * @returns {Uint8Array}
     */
    toBytes() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdf_toBytes(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmPdf.prototype[Symbol.dispose] = WasmPdf.prototype.free;

/**
 * A PDF document loaded from bytes for use in WebAssembly.
 *
 * Create an instance by passing PDF file bytes to the constructor.
 * Call `.free()` when done to release memory.
 */
export class WasmPdfDocument {
    static __wrap(ptr) {
        const obj = Object.create(WasmPdfDocument.prototype);
        obj.__wbg_ptr = ptr;
        WasmPdfDocumentFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmPdfDocumentFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmpdfdocument_free(ptr, 0);
    }
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Add an image stamp annotation (logo watermark) to a page.
     * The image bytes (JPEG/PNG) are embedded as an XObject in the appearance stream.
     * Removal trigger: upstream adds image stamp to WasmPdfDocument.
     * @param {number} page_index
     * @param {Uint8Array} image_data
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     * @param {number} opacity
     */
    addImageStamp(page_index, image_data, x, y, width, height, opacity) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(image_data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_addImageStamp(retptr, this.__wbg_ptr, page_index, ptr0, len0, x, y, width, height, opacity);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Add a stamp annotation to a page.
     * Removal trigger: upstream adds stamp annotations to WasmPdfDocument.
     * @param {number} page_index
     * @param {number} stamp_type
     * @param {string | null | undefined} custom_name
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     * @param {number} opacity
     */
    addStamp(page_index, stamp_type, custom_name, x, y, width, height, opacity) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(custom_name) ? 0 : passStringToWasm0(custom_name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_addStamp(retptr, this.__wbg_ptr, page_index, stamp_type, ptr0, len0, x, y, width, height, opacity);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Add a text watermark to an existing page.
     * Removal trigger: upstream adds watermark to WasmPdfDocument.
     * @param {number} page_index
     * @param {string} text
     * @param {number} font_size
     * @param {number} rotation
     * @param {number} opacity
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    addWatermark(page_index, text, font_size, rotation, opacity, r, g, b) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_addWatermark(retptr, this.__wbg_ptr, page_index, ptr0, len0, font_size, rotation, opacity, r, g, b);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Add a positioned watermark (text at specific coordinates) to a page.
     * Removal trigger: upstream adds positioned watermark to WasmPdfDocument.
     * @param {number} page_index
     * @param {string} text
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     * @param {number} font_size
     * @param {string | null | undefined} font_name
     * @param {number} rotation
     * @param {number} opacity
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    addWatermarkPositioned(page_index, text, x, y, width, height, font_size, font_name, rotation, opacity, r, g, b) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            var ptr1 = isLikeNone(font_name) ? 0 : passStringToWasm0(font_name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_addWatermarkPositioned(retptr, this.__wbg_ptr, page_index, ptr0, len0, x, y, width, height, font_size, ptr1, len1, rotation, opacity, r, g, b);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Apply all redactions in the document.
     */
    applyAllRedactions() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_applyAllRedactions(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Apply redactions on a page (removes redacted content permanently).
     * @param {number} page_index
     */
    applyPageRedactions(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_applyPageRedactions(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Authenticate with a password to decrypt an encrypted PDF.
     *
     * @param password - The password string
     * @returns true if authentication succeeded
     * @param {string} password
     * @returns {boolean}
     */
    authenticate(password) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(password, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_authenticate(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 !== 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Clear all pending erase operations for a page.
     * @param {number} page_index
     */
    clearEraseRegions(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_clearEraseRegions(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Convert the document to PDF/A compliance.
     *
     * Level must be one of: `"1a"`, `"1b"`, `"2a"`, `"2b"`, `"2u"`, `"3a"`, `"3b"`, `"3u"`.
     * Returns a JS object with `success`, `level`, `actions`, and `errors` fields.
     * @param {string} level
     * @returns {any}
     */
    convertToPdfA(level) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(level, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_convertToPdfA(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Crop margins from all pages.
     * @param {number} left
     * @param {number} right
     * @param {number} top
     * @param {number} bottom
     */
    cropMargins(left, right, top, bottom) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_cropMargins(retptr, this.__wbg_ptr, left, right, top, bottom);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Delete a page by index (0-based).
     * @param {number} index
     */
    deletePage(index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_deletePage(retptr, this.__wbg_ptr, index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Deprecated: Use eraseFooter instead.
     * @param {number} page_index
     */
    editFooter(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_editFooter(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Deprecated: Use eraseHeader instead.
     * @param {number} page_index
     */
    editHeader(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_editHeader(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Embed a file into the PDF document.
     *
     * @param name - Display name for the embedded file
     * @param data - File contents as a Uint8Array
     * @param {string} name
     * @param {Uint8Array} data
     */
    embedFile(name, data) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len1 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_embedFile(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Erase both header and footer content.
     *
     * @param page_index - Zero-based page number
     * @param {number} page_index
     */
    eraseArtifacts(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_eraseArtifacts(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Erase existing footer content.
     *
     * Identifies existing text in the footer area (bottom 15%) and marks it for erasure.
     *
     * @param page_index - Zero-based page number
     * @param {number} page_index
     */
    eraseFooter(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_eraseFooter(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Erase existing header content.
     *
     * Identifies existing text in the header area (top 15%) and marks it for erasure.
     *
     * @param page_index - Zero-based page number
     * @param {number} page_index
     */
    eraseHeader(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_eraseHeader(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Erase (whiteout) a rectangular region on a page.
     * @param {number} page_index
     * @param {number} llx
     * @param {number} lly
     * @param {number} urx
     * @param {number} ury
     */
    eraseRegion(page_index, llx, lly, urx, ury) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_eraseRegion(retptr, this.__wbg_ptr, page_index, llx, lly, urx, ury);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Erase multiple rectangular regions on a page.
     *
     * @param page_index - Zero-based page number
     * @param rects - Flat array of coordinates [llx1,lly1,urx1,ury1, llx2,lly2,urx2,ury2, ...]
     * @param {number} page_index
     * @param {Float32Array} rects
     */
    eraseRegions(page_index, rects) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(rects, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_eraseRegions(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Export form field data as FDF or XFDF bytes.
     *
     * @param format - "fdf" or "xfdf" (default: "fdf")
     * @returns Uint8Array containing the exported form data
     * @param {string | null} [format]
     * @returns {Uint8Array}
     */
    exportFormData(format) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(format) ? 0 : passStringToWasm0(format, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_exportFormData(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v2 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v2;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract plain text from all pages, separated by form feed characters.
     * @returns {string}
     */
    extractAllText() {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_extractAllText(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Extract character-level data from a page.
     *
     * Returns an array of objects with: char, bbox {x, y, width, height},
     * font_name, font_size, font_weight, is_italic, color {r, g, b}, etc.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @param {number} page_index
     * @param {Float32Array | null} [region]
     * @returns {any}
     */
    extractChars(page_index, region) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(region) ? 0 : passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractChars(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract image bytes from a page as PNG data.
     *
     * Returns an array of objects with: width, height, data (Uint8Array of PNG bytes), format ("png").
     * @param {number} page_index
     * @returns {any}
     */
    extractImageBytes(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_extractImageBytes(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract image metadata from a page.
     *
     * Returns an array of objects with: width, height, color_space,
     * bits_per_component, bbox (if available). Does NOT return raw image bytes.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @param {number} page_index
     * @param {Float32Array | null} [region]
     * @returns {any}
     */
    extractImages(page_index, region) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(region) ? 0 : passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractImages(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract only straight lines from a page (v0.3.14).
     *
     * Identifies paths that form a single straight line segment.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @returns Array of path objects
     * @param {number} page_index
     * @param {Float32Array | null} [region]
     * @returns {any}
     */
    extractLines(page_index, region) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(region) ? 0 : passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractLines(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract complete page text data in a single call.
     *
     * Returns `{ spans, chars, page_width, page_height }`.
     * The `chars` are derived from spans using font-metric widths when available.
     *
     * Optional `reading_order`: `"column_aware"` for XY-Cut column detection,
     * or `"top_to_bottom"` (default).
     * @param {number} page_index
     * @param {string | null} [reading_order]
     * @returns {any}
     */
    extractPageText(page_index, reading_order) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(reading_order) ? 0 : passStringToWasm0(reading_order, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractPageText(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract specific pages to a new PDF (returns bytes).
     * @param {Uint32Array} pages
     * @returns {Uint8Array}
     */
    extractPages(pages) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray32ToWasm0(pages, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractPages(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v2 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v2;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract vector paths (lines, curves, shapes) from a page.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @returns Array of path objects with bbox, stroke_color, fill_color, etc.
     * @param {number} page_index
     * @param {Float32Array | null} [region]
     * @returns {any}
     */
    extractPaths(page_index, region) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(region) ? 0 : passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractPaths(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract only rectangles from a page (v0.3.14).
     *
     * Identifies paths that form axis-aligned rectangles.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @returns Array of path objects
     * @param {number} page_index
     * @param {Float32Array | null} [region]
     * @returns {any}
     */
    extractRects(page_index, region) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(region) ? 0 : passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractRects(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract span-level data from a page.
     *
     * Returns an array of objects with: text, bbox, font_name, font_size,
     * font_weight, is_italic, color, etc.
     *
     * Optional `reading_order`: `"column_aware"` for XY-Cut column detection,
     * or `"top_to_bottom"` (default).
     * @param {number} page_index
     * @param {Float32Array | null} [region]
     * @param {string | null} [reading_order]
     * @returns {any}
     */
    extractSpans(page_index, region, reading_order) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(region) ? 0 : passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            var len0 = WASM_VECTOR_LEN;
            var ptr1 = isLikeNone(reading_order) ? 0 : passStringToWasm0(reading_order, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractSpans(retptr, this.__wbg_ptr, page_index, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract tables from a page (v0.3.14).
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @param {number} page_index
     * @param {Float32Array | null} [region]
     * @returns {any}
     */
    extractTables(page_index, region) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(region) ? 0 : passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractTables(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract plain text from a single page.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @param {number} page_index
     * @param {any} region
     * @returns {string}
     */
    extractText(page_index, region) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_extractText(retptr, this.__wbg_ptr, page_index, addHeapObject(region));
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Extract text lines from a page.
     *
     * Returns an array of objects with: text, bbox, words (array of Word objects).
     * @param {number} page_index
     * @param {Float32Array | null} [region]
     * @returns {any}
     */
    extractTextLines(page_index, region) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(region) ? 0 : passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractTextLines(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract text using OCR (optical character recognition).
     *
     * NOTE: OCR is not yet supported in the WebAssembly build due to missing
     * ONNX Runtime support for the web backend in the current implementation.
     * @param {number} _page_index
     * @param {WasmOcrEngine | null} [_engine]
     * @returns {string}
     */
    extractTextOcr(_page_index, _engine) {
        let deferred3_0;
        let deferred3_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            let ptr0 = 0;
            if (!isLikeNone(_engine)) {
                _assertClass(_engine, WasmOcrEngine);
                ptr0 = _engine.__destroy_into_raw();
            }
            wasm.wasmpdfdocument_extractTextOcr(retptr, this.__wbg_ptr, _page_index, ptr0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr2 = r0;
            var len2 = r1;
            if (r3) {
                ptr2 = 0; len2 = 0;
                throw takeObject(r2);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Extract word-level data from a page.
     *
     * Returns an array of objects with: text, bbox, font_name, font_size,
     * font_weight, is_italic, is_bold.
     * @param {number} page_index
     * @param {Float32Array | null} [region]
     * @returns {any}
     */
    extractWords(page_index, region) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(region) ? 0 : passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_extractWords(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Flatten all annotations in the document into page content.
     */
    flattenAllAnnotations() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_flattenAllAnnotations(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Flatten all form fields into page content.
     *
     * After flattening, form field values become static text and are no longer editable.
     */
    flattenForms() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_flattenForms(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Flatten form fields on a specific page.
     *
     * @param page_index - Zero-based page number
     * @param {number} page_index
     */
    flattenFormsOnPage(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_flattenFormsOnPage(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Flatten annotations on a page into the page content.
     * @param {number} page_index
     */
    flattenPageAnnotations(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_flattenPageAnnotations(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Return warnings collected during the last form-flattening save.
     *
     * Each entry names a widget field that had no `/AP` appearance stream;
     * flattening such a field produces a blank rectangle.
     *
     * @returns Array of warning strings
     * @returns {string[]}
     */
    flattenWarnings() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_flattenWarnings(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayJsValueFromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 4, 4);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Load a PDF document from a callback-based reader (OPFS).
     *
     * @param read_fn - JS function: (offset: number, count: number) => Uint8Array
     * @param length_fn - JS function: () => number (total file size)
     * @param password - Optional password for encrypted PDFs
     * @param {Function} read_fn
     * @param {Function} length_fn
     * @param {string | null} [password]
     * @returns {WasmPdfDocument}
     */
    static fromReader(read_fn, length_fn, password) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(password) ? 0 : passStringToWasm0(password, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_fromReader(retptr, addHeapObject(read_fn), addHeapObject(length_fn), ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdfDocument.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get annotations from a page.
     *
     * @param page_index - Zero-based page number
     * @returns Array of annotation objects with fields like subtype, rect, contents, etc.
     * @param {number} page_index
     * @returns {any}
     */
    getAnnotations(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_getAnnotations(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get the value of a specific form field by name.
     *
     * @param name - Full qualified field name (e.g., "name" or "topmostSubform[0].Page1[0].f1_01[0]")
     * @returns The field value: string for text, boolean for checkbox, null if not found
     * @param {string} name
     * @returns {any}
     */
    getFormFieldValue(name) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_getFormFieldValue(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get all form fields from the document.
     *
     * Returns an array of form field objects, each with:
     * - name: Full qualified field name
     * - field_type: "text", "button", "choice", "signature", or "unknown"
     * - value: string, boolean, array of strings, or null
     * - tooltip: string or null
     * - bounds: [x1, y1, x2, y2] or null
     * - flags: number or null
     * - max_length: number or null
     * - is_readonly: boolean
     * - is_required: boolean
     * @returns {any}
     */
    getFormFields() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_getFormFields(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get the document outline (bookmarks / table of contents).
     *
     * @returns Array of outline items or null if no outline exists.
     * Each item has: { title, page (number|null), dest_name (string, optional), children (array) }
     * @returns {any}
     */
    getOutline() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_getOutline(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Check if the document has a structure tree (Tagged PDF).
     * @returns {boolean}
     */
    hasStructureTree() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_hasStructureTree(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 !== 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Check if the document contains XFA form data.
     *
     * @returns true if the document has XFA form data
     * @returns {boolean}
     */
    hasXfa() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_hasXfa(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 !== 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Merge another PDF (provided as bytes) into this document.
     *
     * @param data - The PDF file contents to merge as a Uint8Array
     * @returns Number of pages merged
     * @param {Uint8Array} data
     * @returns {number}
     */
    mergeFrom(data) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_mergeFrom(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 >>> 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Move a page within the document. Zero-based; `from_index` and
     * `to_index` refer to positions **before** the move, matching the
     * Python (`PyPdfDocument.move_page`) / Go (`DocumentEditor.MovePage`) /
     * C# contracts.
     * @param {number} from_index
     * @param {number} to_index
     */
    movePage(from_index, to_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_movePage(retptr, this.__wbg_ptr, from_index, to_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Load a PDF document from raw bytes.
     * @param {Uint8Array} data
     * @param {string | null} [password]
     */
    constructor(data, password) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            var ptr1 = isLikeNone(password) ? 0 : passStringToWasm0(password, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_new(retptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0;
            WasmPdfDocumentFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get the number of pages in the document.
     * @returns {number}
     */
    pageCount() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_pageCount(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 >>> 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get the CropBox of a page as [llx, lly, urx, ury], or null if not set.
     * @param {number} page_index
     * @returns {any}
     */
    pageCropBox(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_pageCropBox(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get information about images on a page.
     *
     * Returns an array of {name, bounds: [x, y, width, height], matrix: [a, b, c, d, e, f]}.
     * @param {number} page_index
     * @returns {any}
     */
    pageImages(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_pageImages(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get page label ranges from the document.
     *
     * @returns Array of {start_page, style, prefix, start_value} objects, or empty array
     * @returns {any}
     */
    pageLabels() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_pageLabels(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get the MediaBox of a page as [llx, lly, urx, ury].
     * @param {number} page_index
     * @returns {Float32Array}
     */
    pageMediaBox(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_pageMediaBox(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayF32FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 4, 4);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get the rotation of a page in degrees (0, 90, 180, 270).
     * @param {number} page_index
     * @returns {number}
     */
    pageRotation(page_index) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_pageRotation(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Identify and remove both headers and footers.
     *
     * Prioritizes ISO 32000 spec-compliant /Artifact tags, with a heuristic
     * fallback for untagged PDFs.
     *
     * @param threshold - Fraction of pages (0.0-1.0) where text must repeat (heuristic mode)
     * @param {number} threshold
     * @returns {number}
     */
    removeArtifacts(threshold) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_removeArtifacts(retptr, this.__wbg_ptr, threshold);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 >>> 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Identify and remove footers.
     *
     * Uses spec-compliant /Artifact tags when available (100% accuracy), or
     * falls back to heuristic analysis of the bottom 15% of pages.
     *
     * @param threshold - Fraction of pages (0.0-1.0) where text must repeat (heuristic mode)
     * @param {number} threshold
     * @returns {number}
     */
    removeFooters(threshold) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_removeFooters(retptr, this.__wbg_ptr, threshold);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 >>> 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Identify and remove headers.
     *
     * Uses spec-compliant /Artifact tags when available (100% accuracy), or
     * falls back to heuristic analysis of the top 15% of pages.
     *
     * @param threshold - Fraction of pages (0.0-1.0) where text must repeat (heuristic mode)
     * @param {number} threshold
     * @returns {number}
     */
    removeHeaders(threshold) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_removeHeaders(retptr, this.__wbg_ptr, threshold);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 >>> 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Reposition an image on a page.
     * @param {number} page_index
     * @param {string} name
     * @param {number} x
     * @param {number} y
     */
    repositionImage(page_index, name, x, y) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_repositionImage(retptr, this.__wbg_ptr, page_index, ptr0, len0, x, y);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Resize an image on a page.
     * @param {number} page_index
     * @param {string} name
     * @param {number} width
     * @param {number} height
     */
    resizeImage(page_index, name, width, height) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_resizeImage(retptr, this.__wbg_ptr, page_index, ptr0, len0, width, height);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Rotate all pages by the given degrees.
     * @param {number} degrees
     */
    rotateAllPages(degrees) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_rotateAllPages(retptr, this.__wbg_ptr, degrees);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Rotate a page by the given degrees (adds to current rotation).
     * @param {number} page_index
     * @param {number} degrees
     */
    rotatePage(page_index, degrees) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_rotatePage(retptr, this.__wbg_ptr, page_index, degrees);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Save all edits and return the resulting PDF as bytes.
     *
     * @returns Uint8Array containing the modified PDF
     * @returns {Uint8Array}
     */
    save() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_save(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Save with encryption and return the resulting PDF as bytes.
     * @param {string} user_password
     * @param {string | null} [owner_password]
     * @param {boolean | null} [allow_print]
     * @param {boolean | null} [allow_copy]
     * @param {boolean | null} [allow_modify]
     * @param {boolean | null} [allow_annotate]
     * @returns {Uint8Array}
     */
    saveEncryptedToBytes(user_password, owner_password, allow_print, allow_copy, allow_modify, allow_annotate) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(user_password, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            var ptr1 = isLikeNone(owner_password) ? 0 : passStringToWasm0(owner_password, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_saveEncryptedToBytes(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, isLikeNone(allow_print) ? 0xFFFFFF : allow_print ? 1 : 0, isLikeNone(allow_copy) ? 0xFFFFFF : allow_copy ? 1 : 0, isLikeNone(allow_modify) ? 0xFFFFFF : allow_modify ? 1 : 0, isLikeNone(allow_annotate) ? 0xFFFFFF : allow_annotate ? 1 : 0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v3 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v3;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Save the modified PDF and return as bytes.
     * `saveToBytes()` is the original method; `save()` is a convenience alias.
     *
     * @returns Uint8Array containing the modified PDF
     * @returns {Uint8Array}
     */
    saveToBytes() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_saveToBytes(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Save with options (compress, garbage_collect, linearize) and return bytes.
     *
     * @param {Object} [options] - Optional save options.
     * @param {boolean} [options.compress=true] - Compress raw streams with FlateDecode.
     * @param {boolean} [options.garbageCollect=true] - Remove unreachable objects.
     * @param {boolean} [options.linearize=false] - Linearize (reserved, no-op).
     * @returns Uint8Array containing the modified PDF
     * @param {boolean | null} [compress]
     * @param {boolean | null} [garbage_collect]
     * @param {boolean | null} [linearize]
     * @returns {Uint8Array}
     */
    saveWithOptions(compress, garbage_collect, linearize) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_saveWithOptions(retptr, this.__wbg_ptr, isLikeNone(compress) ? 0xFFFFFF : compress ? 1 : 0, isLikeNone(garbage_collect) ? 0xFFFFFF : garbage_collect ? 1 : 0, isLikeNone(linearize) ? 0xFFFFFF : linearize ? 1 : 0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Search for text across all pages.
     *
     * @param pattern - Regex pattern or literal text to search for
     * @param case_insensitive - Case insensitive search (default: false)
     * @param literal - Treat pattern as literal text, not regex (default: false)
     * @param whole_word - Match whole words only (default: false)
     * @param max_results - Maximum results to return, 0 = unlimited (default: 0)
     *
     * Returns an array of {page, text, bbox, start_index, end_index, span_boxes}.
     * @param {string} pattern
     * @param {boolean | null} [case_insensitive]
     * @param {boolean | null} [literal]
     * @param {boolean | null} [whole_word]
     * @param {number | null} [max_results]
     * @returns {any}
     */
    search(pattern, case_insensitive, literal, whole_word, max_results) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(pattern, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_search(retptr, this.__wbg_ptr, ptr0, len0, isLikeNone(case_insensitive) ? 0xFFFFFF : case_insensitive ? 1 : 0, isLikeNone(literal) ? 0xFFFFFF : literal ? 1 : 0, isLikeNone(whole_word) ? 0xFFFFFF : whole_word ? 1 : 0, isLikeNone(max_results) ? Number.MAX_SAFE_INTEGER : (max_results) >>> 0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Search for text on a specific page.
     * @param {number} page_index
     * @param {string} pattern
     * @param {boolean | null} [case_insensitive]
     * @param {boolean | null} [literal]
     * @param {boolean | null} [whole_word]
     * @param {number | null} [max_results]
     * @returns {any}
     */
    searchPage(page_index, pattern, case_insensitive, literal, whole_word, max_results) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(pattern, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_searchPage(retptr, this.__wbg_ptr, page_index, ptr0, len0, isLikeNone(case_insensitive) ? 0xFFFFFF : case_insensitive ? 1 : 0, isLikeNone(literal) ? 0xFFFFFF : literal ? 1 : 0, isLikeNone(whole_word) ? 0xFFFFFF : whole_word ? 1 : 0, isLikeNone(max_results) ? Number.MAX_SAFE_INTEGER : (max_results) >>> 0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the document author.
     * @param {string} author
     */
    setAuthor(author) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(author, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_setAuthor(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the value of a form field.
     *
     * @param name - Full qualified field name
     * @param value - New value: string for text fields, boolean for checkboxes
     * @param {string} name
     * @param {any} value
     */
    setFormFieldValue(name, value) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_setFormFieldValue(retptr, this.__wbg_ptr, ptr0, len0, addHeapObject(value));
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the complete bounds of an image on a page.
     * @param {number} page_index
     * @param {string} name
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     */
    setImageBounds(page_index, name, x, y, width, height) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_setImageBounds(retptr, this.__wbg_ptr, page_index, ptr0, len0, x, y, width, height);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the document keywords.
     * @param {string} keywords
     */
    setKeywords(keywords) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(keywords, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_setKeywords(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the CropBox of a page.
     * @param {number} page_index
     * @param {number} llx
     * @param {number} lly
     * @param {number} urx
     * @param {number} ury
     */
    setPageCropBox(page_index, llx, lly, urx, ury) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_setPageCropBox(retptr, this.__wbg_ptr, page_index, llx, lly, urx, ury);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the MediaBox of a page.
     * @param {number} page_index
     * @param {number} llx
     * @param {number} lly
     * @param {number} urx
     * @param {number} ury
     */
    setPageMediaBox(page_index, llx, lly, urx, ury) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_setPageMediaBox(retptr, this.__wbg_ptr, page_index, llx, lly, urx, ury);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the rotation of a page (0, 90, 180, or 270 degrees).
     * @param {number} page_index
     * @param {number} degrees
     */
    setPageRotation(page_index, degrees) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_setPageRotation(retptr, this.__wbg_ptr, page_index, degrees);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the document subject.
     * @param {string} subject
     */
    setSubject(subject) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(subject, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_setSubject(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set the document title.
     * @param {string} title
     */
    setTitle(title) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(title, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_setTitle(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Count existing PDF signatures. Returns 0 when the document has
     * no AcroForm or no signed signature fields.
     * @returns {number}
     */
    signatureCount() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_signatureCount(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 >>> 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Enumerate existing PDF signatures. Each entry is a
     * `WasmSignature` (inspection-only) mirroring the C# and Python
     * Signature surfaces.
     * @returns {WasmSignature[]}
     */
    signatures() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_signatures(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayJsValueFromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 4, 4);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Convert a single page to HTML.
     *
     * @param page_index - Zero-based page number
     * @param preserve_layout - Use CSS positioning to preserve layout (default: false)
     * @param detect_headings - Whether to detect headings (default: true)
     * @param {number} page_index
     * @param {boolean | null} [preserve_layout]
     * @param {boolean | null} [detect_headings]
     * @param {boolean | null} [include_form_fields]
     * @returns {string}
     */
    toHtml(page_index, preserve_layout, detect_headings, include_form_fields) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_toHtml(retptr, this.__wbg_ptr, page_index, isLikeNone(preserve_layout) ? 0xFFFFFF : preserve_layout ? 1 : 0, isLikeNone(detect_headings) ? 0xFFFFFF : detect_headings ? 1 : 0, isLikeNone(include_form_fields) ? 0xFFFFFF : include_form_fields ? 1 : 0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Convert all pages to HTML.
     * @param {boolean | null} [preserve_layout]
     * @param {boolean | null} [detect_headings]
     * @param {boolean | null} [include_form_fields]
     * @returns {string}
     */
    toHtmlAll(preserve_layout, detect_headings, include_form_fields) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_toHtmlAll(retptr, this.__wbg_ptr, isLikeNone(preserve_layout) ? 0xFFFFFF : preserve_layout ? 1 : 0, isLikeNone(detect_headings) ? 0xFFFFFF : detect_headings ? 1 : 0, isLikeNone(include_form_fields) ? 0xFFFFFF : include_form_fields ? 1 : 0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Convert a single page to Markdown.
     *
     * @param page_index - Zero-based page number
     * @param detect_headings - Whether to detect headings (default: true)
     * @param include_images - Whether to include images (default: false)
     * @param {number} page_index
     * @param {boolean | null} [detect_headings]
     * @param {boolean | null} [include_images]
     * @param {boolean | null} [include_form_fields]
     * @returns {string}
     */
    toMarkdown(page_index, detect_headings, include_images, include_form_fields) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_toMarkdown(retptr, this.__wbg_ptr, page_index, isLikeNone(detect_headings) ? 0xFFFFFF : detect_headings ? 1 : 0, isLikeNone(include_images) ? 0xFFFFFF : include_images ? 1 : 0, isLikeNone(include_form_fields) ? 0xFFFFFF : include_form_fields ? 1 : 0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Convert all pages to Markdown.
     * @param {boolean | null} [detect_headings]
     * @param {boolean | null} [include_images]
     * @param {boolean | null} [include_form_fields]
     * @returns {string}
     */
    toMarkdownAll(detect_headings, include_images, include_form_fields) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_toMarkdownAll(retptr, this.__wbg_ptr, isLikeNone(detect_headings) ? 0xFFFFFF : detect_headings ? 1 : 0, isLikeNone(include_images) ? 0xFFFFFF : include_images ? 1 : 0, isLikeNone(include_form_fields) ? 0xFFFFFF : include_form_fields ? 1 : 0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Convert a single page to plain text (with layout preservation options).
     * @param {number} page_index
     * @returns {string}
     */
    toPlainText(page_index) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_toPlainText(retptr, this.__wbg_ptr, page_index);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Convert all pages to plain text.
     * @returns {string}
     */
    toPlainTextAll() {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_toPlainTextAll(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Unembed Standard 14 fonts.
     * Removal trigger: upstream adds font optimization to WasmPdfDocument.
     * @returns {number}
     */
    unembedStandardFonts() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_unembedStandardFonts(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 >>> 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Validate PDF/A compliance. Level: "1b", "2b", etc.
     * @param {string} level
     * @returns {any}
     */
    validatePdfA(level) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(level, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_validatePdfA(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Validate PDF/UA accessibility compliance.
     * @param {string | null} [level]
     * @returns {any}
     */
    validatePdfUa(level) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(level) ? 0 : passStringToWasm0(level, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_validatePdfUa(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Validate PDF/X print production compliance.
     * @param {string | null} [level]
     * @returns {any}
     */
    validatePdfX(level) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            var ptr0 = isLikeNone(level) ? 0 : passStringToWasm0(level, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_validatePdfX(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get the PDF version as [major, minor].
     * @returns {Uint8Array}
     */
    version() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_version(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Focus extraction on a specific rectangular region of a page (v0.3.14).
     *
     * @param page_index - Zero-based page number
     * @param region - [x, y, width, height] in points
     * @param {number} page_index
     * @param {Float32Array} region
     * @returns {WasmPdfPageRegion}
     */
    within(page_index, region) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(region, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmpdfdocument_within(retptr, this.__wbg_ptr, page_index, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmPdfPageRegion.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get XMP metadata from the document.
     *
     * @returns Object with XMP fields (dc_title, dc_creator, etc.) or null if no XMP
     * @returns {any}
     */
    xmpMetadata() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfdocument_xmpMetadata(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmPdfDocument.prototype[Symbol.dispose] = WasmPdfDocument.prototype.free;

/**
 * A focused view of a PDF page region for scoped extraction (v0.3.14).
 */
export class WasmPdfPageRegion {
    static __wrap(ptr) {
        const obj = Object.create(WasmPdfPageRegion.prototype);
        obj.__wbg_ptr = ptr;
        WasmPdfPageRegionFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmPdfPageRegionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmpdfpageregion_free(ptr, 0);
    }
    /**
     * Extract character-level data from this region.
     * @returns {any}
     */
    extractChars() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfpageregion_extractChars(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract images from this region.
     * @returns {any}
     */
    extractImages() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfpageregion_extractImages(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract straight lines from this region.
     * @returns {any}
     */
    extractLines() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfpageregion_extractLines(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract vector paths from this region.
     * @returns {any}
     */
    extractPaths() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfpageregion_extractPaths(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract rectangles from this region.
     * @returns {any}
     */
    extractRects() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfpageregion_extractRects(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract tables from this region.
     * @returns {any}
     */
    extractTables() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfpageregion_extractTables(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract text from this region.
     * @returns {string}
     */
    extractText() {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfpageregion_extractText(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Extract text lines from this region.
     * @returns {any}
     */
    extractTextLines() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfpageregion_extractTextLines(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Extract text using OCR from this region.
     * @param {WasmOcrEngine | null} [_engine]
     * @returns {string}
     */
    extractTextOcr(_engine) {
        let deferred3_0;
        let deferred3_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            let ptr0 = 0;
            if (!isLikeNone(_engine)) {
                _assertClass(_engine, WasmOcrEngine);
                ptr0 = _engine.__destroy_into_raw();
            }
            wasm.wasmpdfpageregion_extractTextOcr(retptr, this.__wbg_ptr, ptr0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr2 = r0;
            var len2 = r1;
            if (r3) {
                ptr2 = 0; len2 = 0;
                throw takeObject(r2);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Extract words from this region.
     * @returns {any}
     */
    extractWords() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmpdfpageregion_extractWords(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmPdfPageRegion.prototype[Symbol.dispose] = WasmPdfPageRegion.prototype.free;

/**
 * A single existing PDF signature surfaced by
 * `WasmPdfDocument.signatures()`. `verify()` runs the signer-attributes
 * check; `verifyDetached()` adds the `messageDigest` content-hash check.
 * Supported: RSA-PKCS#1 v1.5, RSA-PSS, ECDSA P-256/P-384.
 */
export class WasmSignature {
    static __wrap(ptr) {
        const obj = Object.create(WasmSignature.prototype);
        obj.__wbg_ptr = ptr;
        WasmSignatureFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmSignatureFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmsignature_free(ptr, 0);
    }
    /**
     * `/ContactInfo` entry from the signature dictionary, if present.
     * @returns {string | undefined}
     */
    get contactInfo() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmsignature_contactInfo(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            let v1;
            if (r0 !== 0) {
                v1 = getStringFromWasm0(r0, r1).slice();
                wasm.__wbindgen_export4(r0, r1 * 1, 1);
            }
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * True iff `/ByteRange` is a 4-element array covering the whole
     * document (i.e. the signature protects every byte of the file).
     * @returns {boolean}
     */
    get coversWholeDocument() {
        const ret = wasm.wasmsignature_coversWholeDocument(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * `/Location` entry from the signature dictionary, if present.
     * @returns {string | undefined}
     */
    get location() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmsignature_location(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            let v1;
            if (r0 !== 0) {
                v1 = getStringFromWasm0(r0, r1).slice();
                wasm.__wbindgen_export4(r0, r1 * 1, 1);
            }
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * `/Reason` entry from the signature dictionary, if present.
     * @returns {string | undefined}
     */
    get reason() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmsignature_reason(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            let v1;
            if (r0 !== 0) {
                v1 = getStringFromWasm0(r0, r1).slice();
                wasm.__wbindgen_export4(r0, r1 * 1, 1);
            }
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * `/Name` entry from the signature dictionary, if present.
     * @returns {string | undefined}
     */
    get signerName() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmsignature_signerName(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            let v1;
            if (r0 !== 0) {
                v1 = getStringFromWasm0(r0, r1).slice();
                wasm.__wbindgen_export4(r0, r1 * 1, 1);
            }
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Unix epoch (seconds). `None` if the `/M` entry is missing or
     * unparseable.
     * @returns {bigint | undefined}
     */
    get signingTime() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmsignature_signingTime(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r2 = getDataViewMemory0().getBigInt64(retptr + 8 * 1, true);
            return r0 === 0 ? undefined : r2;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Run the RFC 5652 §5.4 signer-attributes crypto check. Today
     * this covers RSA-PKCS#1 v1.5 over SHA-1/256/384/512 — the
     * padding used by essentially every PDF signature.
     *
     * A `true` return proves the signer held the private key matching
     * the embedded certificate and that the signed-attribute bundle
     * is authentic. It does **not** verify the `messageDigest`
     * attribute against the document's byte-range content hash —
     * call `verifyDetached()` for that end-to-end check.
     *
     * Throws for RSA-PSS, ECDSA, unknown digest OIDs, or signatures
     * without signed_attrs.
     * @returns {boolean}
     */
    verify() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmsignature_verify(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 !== 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * End-to-end detached-signature verification. Runs both the
     * signer-attributes RSA-PKCS#1 v1.5 crypto check AND the RFC 5652
     * §11.2 `messageDigest` check against the segment of `pdfData`
     * this signature covers (extracted via `/ByteRange`).
     *
     * `pdfData` must be the full PDF file. A `true` result proves
     * both the signer is authentic and that the document's byte-range
     * content has not been altered since signing. `false` means one
     * of the two checks failed (wrong key or tampered content).
     *
     * Throws for RSA-PSS, ECDSA, unknown digest OIDs, or CMS blobs
     * missing `signed_attrs` / `messageDigest`.
     * @param {Uint8Array} pdf_data
     * @returns {boolean}
     */
    verifyDetached(pdf_data) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(pdf_data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmsignature_verifyDetached(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 !== 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmSignature.prototype[Symbol.dispose] = WasmSignature.prototype.free;

/**
 * RFC 3161 timestamp parsed from a DER TimeStampToken or bare
 * TSTInfo. Mirrors the C#, Go, and Python `Timestamp` surfaces.
 */
export class WasmTimestamp {
    static __wrap(ptr) {
        const obj = Object.create(WasmTimestamp.prototype);
        obj.__wbg_ptr = ptr;
        WasmTimestampFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmTimestampFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmtimestamp_free(ptr, 0);
    }
    /**
     * Hash algorithm enum value (1=SHA1, 2=SHA256, 3=SHA384,
     * 4=SHA512, 0=unknown).
     * @returns {number}
     */
    get hashAlgorithm() {
        const ret = wasm.wasmtimestamp_hashAlgorithm(this.__wbg_ptr);
        return ret;
    }
    /**
     * Raw message-imprint hash bytes.
     * @returns {Uint8Array}
     */
    get messageImprint() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmtimestamp_messageImprint(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Parse a DER blob that may be either a full TimeStampToken or
     * the bare TSTInfo SEQUENCE.
     * @param {Uint8Array} data
     * @returns {WasmTimestamp}
     */
    static parse(data) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmtimestamp_parse(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return WasmTimestamp.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * TSA policy OID in dotted-decimal form.
     * @returns {string}
     */
    get policyOid() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmtimestamp_policyOid(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Serial number as a hex string (no `0x` prefix).
     * @returns {string}
     */
    get serial() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmtimestamp_serial(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Generation time as Unix epoch seconds.
     * @returns {bigint}
     */
    get time() {
        const ret = wasm.wasmtimestamp_time(this.__wbg_ptr);
        return ret;
    }
    /**
     * TSA name from the token (may be empty).
     * @returns {string}
     */
    get tsaName() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmtimestamp_tsaName(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Cryptographically verify this TimeStampToken.
     *
     * Returns `true` when the TSA's signature and `messageDigest` both pass.
     * Returns `false` when a crypto check fails (tampered token or wrong key).
     * Throws when the token is not CMS-wrapped or uses an unsupported algorithm.
     * @returns {boolean}
     */
    verify() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmtimestamp_verify(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 !== 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmTimestamp.prototype[Symbol.dispose] = WasmTimestamp.prototype.free;

/**
 * Disable all pdf_oxide log output — convenience wrapper for
 * `setLogLevel("off")`.
 */
export function disableLogging() {
    wasm.disableLogging();
}

/**
 * Generate a 1D barcode as an SVG string.
 *
 * `barcodeType`: 0=Code128, 1=Code39, 2=EAN13, 3=EAN8, 4=UPCA, 5=ITF, 6=Code93, 7=Codabar.
 * @param {number} barcode_type
 * @param {string} data
 * @returns {string}
 */
export function generateBarcodeSvg(barcode_type, data) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(data, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        wasm.generateBarcodeSvg(retptr, barcode_type, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
        var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
        var ptr2 = r0;
        var len2 = r1;
        if (r3) {
            ptr2 = 0; len2 = 0;
            throw takeObject(r2);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export4(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Generate a QR code as an SVG string.
 *
 * `errorCorrection`: 0=Low, 1=Medium, 2=Quartile, 3=High. `size`: advisory pixel size.
 * @param {string} data
 * @param {number} error_correction
 * @param {number} size
 * @returns {string}
 */
export function generateQrSvg(data, error_correction, size) {
    let deferred3_0;
    let deferred3_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(data, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        wasm.generateQrSvg(retptr, ptr0, len0, error_correction, size);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
        var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
        var ptr2 = r0;
        var len2 = r1;
        if (r3) {
            ptr2 = 0; len2 = 0;
            throw takeObject(r2);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export4(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Set the maximum log level for pdf_oxide messages.
 *
 * Accepts one of: `"off"`, `"error"`, `"warn"` / `"warning"`, `"info"`,
 * `"debug"`, `"trace"`. Case-insensitive. Default is `"off"` — the library
 * is silent unless explicitly enabled.
 *
 * Logs are forwarded to the browser console (`console.log`, `console.warn`,
 * `console.error`, etc.). Fixes issue #280.
 *
 * @example
 * ```javascript
 * import init, { setLogLevel } from "pdf-oxide-wasm";
 * await init();
 * setLogLevel("warn");
 * ```
 * @param {string} level
 */
export function setLogLevel(level) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(level, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        wasm.setLogLevel(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        if (r1) {
            throw takeObject(r0);
        }
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
 * Sign raw PDF bytes and return the signed PDF as a `Uint8Array`.
 *
 * `cert` must carry a private key (loaded via `Certificate.loadPem` or
 * `Certificate.loadPkcs12`).
 * @param {Uint8Array} pdf_data
 * @param {WasmCertificate} cert
 * @param {string | null} [reason]
 * @param {string | null} [location]
 * @returns {Uint8Array}
 */
export function signPdfBytes(pdf_data, cert, reason, location) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArray8ToWasm0(pdf_data, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        _assertClass(cert, WasmCertificate);
        var ptr1 = isLikeNone(reason) ? 0 : passStringToWasm0(reason, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(location) ? 0 : passStringToWasm0(location, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        var len2 = WASM_VECTOR_LEN;
        wasm.signPdfBytes(retptr, ptr0, len0, cert.__wbg_ptr, ptr1, len1, ptr2, len2);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
        var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
        if (r3) {
            throw takeObject(r2);
        }
        var v4 = getArrayU8FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 1, 1);
        return v4;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_bce6d499ff0a4aff: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        },
        __wbg_Number_b7972a139bfbfdf0: function(arg0) {
            const ret = Number(getObject(arg0));
            return ret;
        },
        __wbg_String_8564e559799eccda: function(arg0, arg1) {
            const ret = String(getObject(arg1));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_bigint_get_as_i64_410e28c7b761ad83: function(arg0, arg1) {
            const v = getObject(arg1);
            const ret = typeof(v) === 'bigint' ? v : undefined;
            getDataViewMemory0().setBigInt64(arg0 + 8 * 1, isLikeNone(ret) ? BigInt(0) : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_boolean_get_2304fb8c853028c8: function(arg0) {
            const v = getObject(arg0);
            const ret = typeof(v) === 'boolean' ? v : undefined;
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        },
        __wbg___wbindgen_debug_string_edece8177ad01481: function(arg0, arg1) {
            const ret = debugString(getObject(arg1));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_in_07056af4f902c445: function(arg0, arg1) {
            const ret = getObject(arg0) in getObject(arg1);
            return ret;
        },
        __wbg___wbindgen_is_bigint_aeae3893f30ed54e: function(arg0) {
            const ret = typeof(getObject(arg0)) === 'bigint';
            return ret;
        },
        __wbg___wbindgen_is_function_5cd60d5cf78b4eef: function(arg0) {
            const ret = typeof(getObject(arg0)) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_null_2042690d351e14f0: function(arg0) {
            const ret = getObject(arg0) === null;
            return ret;
        },
        __wbg___wbindgen_is_object_b4593df85baada48: function(arg0) {
            const val = getObject(arg0);
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_is_string_dde0fd9020db4434: function(arg0) {
            const ret = typeof(getObject(arg0)) === 'string';
            return ret;
        },
        __wbg___wbindgen_is_undefined_35bb9f4c7fd651d5: function(arg0) {
            const ret = getObject(arg0) === undefined;
            return ret;
        },
        __wbg___wbindgen_jsval_eq_c0ed08b3e0f393b9: function(arg0, arg1) {
            const ret = getObject(arg0) === getObject(arg1);
            return ret;
        },
        __wbg___wbindgen_jsval_loose_eq_0ad77b7717db155c: function(arg0, arg1) {
            const ret = getObject(arg0) == getObject(arg1);
            return ret;
        },
        __wbg___wbindgen_number_get_f73a1244370fcc2c: function(arg0, arg1) {
            const obj = getObject(arg1);
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_d109740c0d18f4d7: function(arg0, arg1) {
            const obj = getObject(arg1);
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_9c31b086c2b26051: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_call_13665d9f14390edc: function() { return handleError(function (arg0, arg1) {
            const ret = getObject(arg0).call(getObject(arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_call_faa0a261f288f846: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = getObject(arg0).call(getObject(arg1), getObject(arg2), getObject(arg3));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_debug_1cbb2f02cd18a348: function(arg0, arg1, arg2, arg3) {
            console.debug(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        },
        __wbg_done_54b8da57023b7ed2: function(arg0) {
            const ret = getObject(arg0).done;
            return ret;
        },
        __wbg_error_a6fa202b58aa1cd3: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_export4(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_error_ff9920154b136cbb: function(arg0, arg1, arg2, arg3) {
            console.error(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        },
        __wbg_fromCodePoint_a42ea1d19a55f2d5: function() { return handleError(function (arg0) {
            const ret = String.fromCodePoint(arg0 >>> 0);
            return addHeapObject(ret);
        }, arguments); },
        __wbg_from_fa561fa561dc8031: function(arg0) {
            const ret = Array.from(getObject(arg0));
            return addHeapObject(ret);
        },
        __wbg_getRandomValues_76dfc69825c9c552: function() { return handleError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_getRandomValues_ef12552bf5acd2fe: function() { return handleError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_getTime_09f1dd40a44edb30: function(arg0) {
            const ret = getObject(arg0).getTime();
            return ret;
        },
        __wbg_getTimezoneOffset_96cfb6ddebc9e5ca: function(arg0) {
            const ret = getObject(arg0).getTimezoneOffset();
            return ret;
        },
        __wbg_get_3e9a707ab7d352eb: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(getObject(arg0), getObject(arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_get_98fdf51d029a75eb: function(arg0, arg1) {
            const ret = getObject(arg0)[arg1 >>> 0];
            return addHeapObject(ret);
        },
        __wbg_get_unchecked_1dfe6d05ad91d9b7: function(arg0, arg1) {
            const ret = getObject(arg0)[arg1 >>> 0];
            return addHeapObject(ret);
        },
        __wbg_get_with_ref_key_6412cf3094599694: function(arg0, arg1) {
            const ret = getObject(arg0)[getObject(arg1)];
            return addHeapObject(ret);
        },
        __wbg_info_de0a30e0c0b6b4e9: function(arg0, arg1, arg2, arg3) {
            console.info(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        },
        __wbg_instanceof_ArrayBuffer_53db37b06f6b9afe: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof ArrayBuffer;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Uint8Array_abd07d4bd221d50b: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof Uint8Array;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_isArray_94898ed3aad6947b: function(arg0) {
            const ret = Array.isArray(getObject(arg0));
            return ret;
        },
        __wbg_isSafeInteger_01e964d144ad3a55: function(arg0) {
            const ret = Number.isSafeInteger(getObject(arg0));
            return ret;
        },
        __wbg_iterator_1441b47f341dc34f: function() {
            const ret = Symbol.iterator;
            return addHeapObject(ret);
        },
        __wbg_length_2591a0f4f659a55c: function(arg0) {
            const ret = getObject(arg0).length;
            return ret;
        },
        __wbg_length_56fcd3e2b7e0299d: function(arg0) {
            const ret = getObject(arg0).length;
            return ret;
        },
        __wbg_log_2a34598952277e1c: function(arg0, arg1, arg2, arg3) {
            console.log(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        },
        __wbg_new_02d162bc6cf02f60: function() {
            const ret = new Object();
            return addHeapObject(ret);
        },
        __wbg_new_070df68d66325372: function() {
            const ret = new Map();
            return addHeapObject(ret);
        },
        __wbg_new_0_2722fcdb71a888a6: function() {
            const ret = new Date();
            return addHeapObject(ret);
        },
        __wbg_new_227d7c05414eb861: function() {
            const ret = new Error();
            return addHeapObject(ret);
        },
        __wbg_new_310879b66b6e95e1: function() {
            const ret = new Array();
            return addHeapObject(ret);
        },
        __wbg_new_7ddec6de44ff8f5d: function(arg0) {
            const ret = new Uint8Array(getObject(arg0));
            return addHeapObject(ret);
        },
        __wbg_new_859b9002e2668e82: function(arg0) {
            const ret = new Date(getObject(arg0));
            return addHeapObject(ret);
        },
        __wbg_new_from_slice_269e35316ed2d061: function(arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return addHeapObject(ret);
        },
        __wbg_next_2a4e19f4f5083b0f: function(arg0) {
            const ret = getObject(arg0).next;
            return addHeapObject(ret);
        },
        __wbg_next_6429a146bf756f93: function() { return handleError(function (arg0) {
            const ret = getObject(arg0).next();
            return addHeapObject(ret);
        }, arguments); },
        __wbg_prototypesetcall_5f9bdc8d75e07276: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), getObject(arg2));
        },
        __wbg_push_b77c476b01548d0a: function(arg0, arg1) {
            const ret = getObject(arg0).push(getObject(arg1));
            return ret;
        },
        __wbg_set_6be42768c690e380: function(arg0, arg1, arg2) {
            getObject(arg0)[takeObject(arg1)] = takeObject(arg2);
        },
        __wbg_set_78ea6a19f4818587: function(arg0, arg1, arg2) {
            getObject(arg0)[arg1 >>> 0] = takeObject(arg2);
        },
        __wbg_set_a0e911be3da02782: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(getObject(arg0), getObject(arg1), getObject(arg2));
            return ret;
        }, arguments); },
        __wbg_set_facb7a5914e0fa39: function(arg0, arg1, arg2) {
            const ret = getObject(arg0).set(getObject(arg1), getObject(arg2));
            return addHeapObject(ret);
        },
        __wbg_stack_3b0d974bbf31e44f: function(arg0, arg1) {
            const ret = getObject(arg1).stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_value_9cc0518af87a489c: function(arg0) {
            const ret = getObject(arg0).value;
            return addHeapObject(ret);
        },
        __wbg_warn_099dd8c85568de56: function(arg0, arg1, arg2, arg3) {
            console.warn(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        },
        __wbg_wasmsignature_new: function(arg0) {
            const ret = WasmSignature.__wrap(arg0);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000001: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000002: function(arg0) {
            // Cast intrinsic for `I64 -> Externref`.
            const ret = arg0;
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000004: function(arg0) {
            // Cast intrinsic for `U64 -> Externref`.
            const ret = BigInt.asUintN(64, arg0);
            return addHeapObject(ret);
        },
        __wbindgen_object_clone_ref: function(arg0) {
            const ret = getObject(arg0);
            return addHeapObject(ret);
        },
        __wbindgen_object_drop_ref: function(arg0) {
            takeObject(arg0);
        },
    };
    return {
        __proto__: null,
        "./pdf_oxide_bg.js": import0,
    };
}

const WasmArtifactFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmartifact_free(ptr, 1));
const ArtifactStyleFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_artifactstyle_free(ptr, 1));
const WasmCertificateFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmcertificate_free(ptr, 1));
const WasmDocumentBuilderFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmdocumentbuilder_free(ptr, 1));
const WasmEmbeddedFontFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmembeddedfont_free(ptr, 1));
const WasmFluentPageBuilderFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmfluentpagebuilder_free(ptr, 1));
const WasmFooterFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmfooter_free(ptr, 1));
const WasmHeaderFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmheader_free(ptr, 1));
const WasmOcrConfigFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmocrconfig_free(ptr, 1));
const WasmOcrEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmocrengine_free(ptr, 1));
const WasmPageTemplateFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmpagetemplate_free(ptr, 1));
const WasmPdfFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmpdf_free(ptr, 1));
const WasmPdfDocumentFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmpdfdocument_free(ptr, 1));
const WasmPdfPageRegionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmpdfpageregion_free(ptr, 1));
const WasmSignatureFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmsignature_free(ptr, 1));
const StreamingTableFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_streamingtable_free(ptr, 1));
const WasmTimestampFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmtimestamp_free(ptr, 1));

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function _assertClass(instance, klass) {
    if (!(instance instanceof klass)) {
        throw new Error(`expected instance of ${klass.name}`);
    }
}

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function dropObject(idx) {
    if (idx < 1028) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayI64FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getBigInt64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(takeObject(mem.getUint32(i, true)));
    }
    return result;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedBigInt64ArrayMemory0 = null;
function getBigInt64ArrayMemory0() {
    if (cachedBigInt64ArrayMemory0 === null || cachedBigInt64ArrayMemory0.byteLength === 0) {
        cachedBigInt64ArrayMemory0 = new BigInt64Array(wasm.memory.buffer);
    }
    return cachedBigInt64ArrayMemory0;
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function getObject(idx) { return heap[idx]; }

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        wasm.__wbindgen_export3(addHeapObject(e));
    }
}

let heap = new Array(1024).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayF32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getFloat32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    const mem = getDataViewMemory0();
    for (let i = 0; i < array.length; i++) {
        mem.setUint32(ptr + 4 * i, addHeapObject(array[i]), true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedBigInt64ArrayMemory0 = null;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('pdf_oxide_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
