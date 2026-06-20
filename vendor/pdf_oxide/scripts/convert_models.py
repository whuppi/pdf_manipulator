#!/usr/bin/env python3
"""
Convert pre-trained transformer models to ONNX format for CPU inference.

This script downloads pre-trained models from HuggingFace, converts them to ONNX,
and quantizes them to INT8 for faster CPU inference.

Note: This script requires python >= 3.10 because of onnxruntime needs it.

Usage:
    python scripts/convert_models.py --model layout-reader
    python scripts/convert_models.py --model heading-classifier
    python scripts/convert_models.py --model all
"""

import argparse
from pathlib import Path

import torch
from onnxruntime.quantization import QuantType, quantize_dynamic
from transformers import AutoModel, AutoModelForSequenceClassification, AutoTokenizer


def convert_layout_reader():
    """Convert LayoutReader model to ONNX"""
    print("Converting LayoutReader model...")
    print("=" * 60)

    # Use LayoutLMv3-base (smaller than full)
    model_name = "microsoft/layoutlmv3-base"
    print(f"Loading model: {model_name}")

    try:
        model = AutoModel.from_pretrained(model_name)
        tokenizer = AutoTokenizer.from_pretrained(model_name)
    except Exception as e:
        print(f"⚠ Failed to download model: {e}")
        print("  Note: This requires internet connection and ~400MB download")
        print("  For testing purposes, you can skip this and use simplified heuristics")
        return False

    # Set to eval mode
    model.eval()

    # Create dummy input
    dummy_input = {
        "input_ids": torch.randint(0, 1000, (1, 512)),
        "bbox": torch.randint(0, 1000, (1, 512, 4)),
        "attention_mask": torch.ones(1, 512, dtype=torch.long),
    }

    # Export to ONNX
    output_path = Path("models/layout_reader.onnx")
    output_path.parent.mkdir(exist_ok=True)

    print("Exporting to ONNX...")
    torch.onnx.export(
        model,
        (dummy_input["input_ids"], dummy_input["bbox"], dummy_input["attention_mask"]),
        str(output_path),
        opset_version=14,
        input_names=["input_ids", "bbox", "attention_mask"],
        output_names=["last_hidden_state"],
        dynamic_axes={
            "input_ids": {0: "batch", 1: "sequence"},
            "bbox": {0: "batch", 1: "sequence"},
            "attention_mask": {0: "batch", 1: "sequence"},
            "last_hidden_state": {0: "batch", 1: "sequence"},
        },
    )

    print(f"✓ Model exported to {output_path}")
    print(f"  Size: {output_path.stat().st_size / 1024 / 1024:.1f} MB")

    # Quantize to INT8 for faster CPU inference
    print("Quantizing to INT8...")
    quantized_path = Path("models/layout_reader_int8.onnx")
    quantize_dynamic(str(output_path), str(quantized_path), weight_type=QuantType.QInt8)

    print(f"✓ Quantized model saved to {quantized_path}")
    print(f"  Size: {quantized_path.stat().st_size / 1024 / 1024:.1f} MB")

    # Save tokenizer
    tokenizer.save_pretrained("models/tokenizer")
    print("✓ Tokenizer saved to models/tokenizer")

    return True


def convert_heading_classifier():
    """Convert fine-tuned BERT for heading classification"""
    print("\nConverting heading classifier...")
    print("=" * 60)

    # For demo, use distilbert-base (smaller, faster)
    # In production, fine-tune on your labeled data
    model_name = "distilbert-base-uncased"
    print(f"Loading model: {model_name}")

    try:
        model = AutoModelForSequenceClassification.from_pretrained(
            model_name,
            num_labels=5,  # H1, H2, H3, Body, Small
        )
        tokenizer = AutoTokenizer.from_pretrained(model_name)
    except Exception as e:
        print(f"⚠ Failed to download model: {e}")
        print("  Note: This requires internet connection and ~250MB download")
        print("  For testing purposes, you can skip this and use rule-based classification")
        return False

    model.eval()

    # Dummy input
    dummy_input = torch.randint(0, 1000, (1, 128))

    # Export
    output_path = Path("models/heading_classifier.onnx")
    print("Exporting to ONNX...")
    torch.onnx.export(
        model,
        (dummy_input,),
        str(output_path),
        opset_version=14,
        input_names=["input_ids"],
        output_names=["logits"],
        dynamic_axes={
            "input_ids": {0: "batch", 1: "sequence"},
            "logits": {0: "batch"},
        },
    )

    print(f"✓ Model exported to {output_path}")
    print(f"  Size: {output_path.stat().st_size / 1024 / 1024:.1f} MB")

    # Quantize
    print("Quantizing to INT8...")
    quantized_path = Path("models/heading_classifier_int8.onnx")
    quantize_dynamic(str(output_path), str(quantized_path), weight_type=QuantType.QInt8)

    print(f"✓ Quantized model saved to {quantized_path}")
    print(f"  Size: {quantized_path.stat().st_size / 1024 / 1024:.1f} MB")

    tokenizer.save_pretrained("models/heading_tokenizer")
    print("✓ Tokenizer saved to models/heading_tokenizer")

    return True


def verify_models():
    """Verify ONNX models can be loaded"""
    print("\nVerifying models...")
    print("=" * 60)

    import onnxruntime as ort

    models = ["models/layout_reader_int8.onnx", "models/heading_classifier_int8.onnx"]

    success = True
    for model_path in models:
        if Path(model_path).exists():
            try:
                session = ort.InferenceSession(model_path, providers=["CPUExecutionProvider"])
                print(f"✓ {model_path} verified")
                print(f"  Inputs: {[i.name for i in session.get_inputs()]}")
                print(f"  Outputs: {[o.name for o in session.get_outputs()]}")
            except Exception as e:
                print(f"✗ {model_path} failed verification: {e}")
                success = False
        else:
            print(f"⚠ {model_path} not found (skipped)")

    return success


def main():
    parser = argparse.ArgumentParser(
        description="Convert models to ONNX",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Convert all models
  python scripts/convert_models.py --model all

  # Convert only layout reader
  python scripts/convert_models.py --model layout-reader

Note:
  - Requires internet connection for first run (downloads ~650MB)
  - Models are cached in ~/.cache/huggingface
  - Conversion takes 5-10 minutes on CPU
  - Final model files are ~70MB total
        """,
    )
    parser.add_argument(
        "--model",
        choices=["layout-reader", "heading-classifier", "all"],
        default="all",
        help="Model to convert",
    )
    parser.add_argument("--skip-verify", action="store_true", help="Skip model verification step")
    args = parser.parse_args()

    print("\n" + "=" * 60)
    print("PDF Library - Model Conversion Script")
    print("=" * 60)

    success = True

    if args.model in ["layout-reader", "all"] and (not convert_layout_reader()):
        success = False

    if args.model in ["heading-classifier", "all"] and (not convert_heading_classifier()):
        success = False

    if (not args.skip_verify) and (not verify_models()):
        success = False

    print("\n" + "=" * 60)
    if success:
        print("✓ Model conversion complete!")
        print("\nNext steps:")
        print("  1. Run: cargo build --features ml")
        print("  2. Test: cargo test --features ml")
        print("\nThe ML models are ready for use.")
    else:
        print("⚠ Model conversion completed with warnings")
        print("\nThe library will fall back to rule-based algorithms.")
        print("For full ML functionality, ensure:")
        print("  - Internet connection is available")
        print("  - pip install -r scripts/requirements.txt is run")
        print("  - Sufficient disk space (~1GB for downloads)")
    print("=" * 60)


if __name__ == "__main__":
    main()
