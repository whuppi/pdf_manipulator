// Barcode SVG generation
// Run: node index.js

import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { generateBarcodeSvg, generateQrCodeSvg } from "pdf-oxide";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.join(__dirname, "output");
fs.mkdirSync(OUT_DIR, { recursive: true });

// 1D barcode — Code 128 SVG (format=0)
const code128Svg = generateBarcodeSvg("PDF-OXIDE-0341", 0, 300);
if (!code128Svg.startsWith("<svg")) throw new Error(`Expected SVG, got: ${code128Svg.slice(0, 40)}`);
const code128Path = path.join(OUT_DIR, "code128.svg");
fs.writeFileSync(code128Path, code128Svg);
console.log(`Written: ${code128Path} (${code128Svg.length} bytes)`);

// 1D barcode — EAN-13 SVG (format=2)
const ean13Svg = generateBarcodeSvg("5901234123457", 2, 300);
if (!ean13Svg.startsWith("<svg")) throw new Error("Expected SVG for EAN-13");
const ean13Path = path.join(OUT_DIR, "ean13.svg");
fs.writeFileSync(ean13Path, ean13Svg);
console.log(`Written: ${ean13Path} (${ean13Svg.length} bytes)`);

// QR code SVG (errorCorrection=1=Medium, sizePx=256)
const qrSvg = generateQrCodeSvg("https://github.com/yfedoseev/pdf_oxide", 1, 256);
if (!qrSvg.startsWith("<svg")) throw new Error("Expected SVG for QR code");
if (!qrSvg.includes("<rect")) throw new Error("QR SVG must contain rect elements");
const qrPath = path.join(OUT_DIR, "qr_code.svg");
fs.writeFileSync(qrPath, qrSvg);
console.log(`Written: ${qrPath} (${qrSvg.length} bytes)`);

console.log("All barcode SVG checks passed.");
process.exit(0);
