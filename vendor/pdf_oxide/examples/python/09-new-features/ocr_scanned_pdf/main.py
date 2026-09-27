# OCR text extraction from scanned PDFs
#
# Prerequisites:
#   pip install 'pdf_oxide[ocr]'
#
#   Then download PaddleOCR-compatible ONNX models and set paths below:
#     DET_MODEL  - text detection model  (e.g. ch_PP-OCRv3_det_infer.onnx)
#     REC_MODEL  - text recognition model (e.g. ch_PP-OCRv3_rec_infer.onnx)
#     DICT_PATH  - character dictionary   (e.g. ppocr_keys_v1.txt)
#
#   Models can be downloaded from PaddleOCR:
#     https://github.com/PaddlePaddle/PaddleOCR/blob/main/doc/doc_en/models_list_en.md
#
# Run: python main.py [scanned.pdf]
#
# zsh users: quote the install command to avoid glob errors:
#   pip install 'pdf_oxide[ocr]'

from __future__ import annotations

import os
import sys

import pdf_oxide


DET_MODEL = os.environ.get("OCR_DET_MODEL", "det.onnx")
REC_MODEL = os.environ.get("OCR_REC_MODEL", "rec.onnx")
DICT_PATH = os.environ.get("OCR_DICT", "en_dict.txt")


def main() -> None:
    pdf_path = sys.argv[1] if len(sys.argv) > 1 else None

    if pdf_path is None:
        print("Usage: python main.py <scanned.pdf>")
        print()
        print("Environment variables:")
        print(f"  OCR_DET_MODEL  path to detection ONNX model  (default: {DET_MODEL})")
        print(f"  OCR_REC_MODEL  path to recognition ONNX model (default: {REC_MODEL})")
        print(f"  OCR_DICT       path to character dictionary    (default: {DICT_PATH})")
        print()
        print("Demonstrating OcrEngine/OcrConfig availability...")
        _demo_availability()
        return

    for model_path in (DET_MODEL, REC_MODEL, DICT_PATH):
        if not os.path.exists(model_path):
            print(f"Model file not found: {model_path}")
            print("Set OCR_DET_MODEL / OCR_REC_MODEL / OCR_DICT environment variables.")
            sys.exit(1)

    config = pdf_oxide.OcrConfig(det_threshold=0.3, rec_threshold=0.5, num_threads=4)
    engine = pdf_oxide.OcrEngine(
        det_model_path=DET_MODEL,
        rec_model_path=REC_MODEL,
        dict_path=DICT_PATH,
        config=config,
    )

    doc = pdf_oxide.PdfDocument(pdf_path)
    total = doc.page_count()
    print(f"Processing {total} page(s) from {pdf_path}")

    for page in range(total):
        text = doc.extract_text_ocr(page, engine=engine)
        print(f"\n--- Page {page + 1} ---")
        print(text[:500] + ("..." if len(text) > 500 else ""))


def _demo_availability() -> None:
    """Confirm OcrEngine and OcrConfig are importable (OCR compiled in)."""
    try:
        _ = pdf_oxide.OcrConfig
        _ = pdf_oxide.OcrEngine
        print("OcrEngine and OcrConfig are available in this build.")
    except AttributeError as exc:
        print(f"OCR NOT available: {exc}")
        print("Rebuild with: maturin develop --features python,ocr")
        sys.exit(1)


if __name__ == "__main__":
    main()
