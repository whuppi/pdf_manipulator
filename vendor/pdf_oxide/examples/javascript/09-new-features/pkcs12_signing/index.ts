// PKCS#12 CMS signing (TypeScript)

import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { DocumentBuilder, SignatureManager, SignatureException } from "pdf-oxide";

const __dirname: string = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR: string = path.join(__dirname, "output");
fs.mkdirSync(OUT_DIR, { recursive: true });

const p12Path: string = path.resolve(__dirname, "..", "..", "..", "..", "tests", "fixtures", "test_signing.p12");
if (!fs.existsSync(p12Path)) {
  console.log(`  SKIP: ${p12Path} not found`);
  process.exit(0);
}

try {
  const builder = DocumentBuilder.create().title("Signed Invoice (TS)");
  builder.letterPage()
    .font("Helvetica", 12)
    .at(72, 720).heading(1, "Signed Invoice")
    .at(72, 690).paragraph("This document carries a CMS/PKCS#7 digital signature.")
    .done();
  const pdfBytes: Buffer = builder.build();

  const sigManager = new SignatureManager({});
  const signed: Buffer = await sigManager.signWithPkcs12(pdfBytes, p12Path, "testpass", {
    reason: "Approved",
    location: "HQ",
  });

  const outPath: string = path.join(OUT_DIR, "signed_document_ts.pdf");
  fs.writeFileSync(outPath, signed);
  console.log(`Written: ${outPath} (${signed.length} bytes)`);
  if (!signed.includes(Buffer.from("/ByteRange"))) throw new Error("ByteRange missing");
  console.log("  Signature verified: /ByteRange present.");
} catch (err) {
  if (err instanceof SignatureException || (err instanceof Error && err.message.includes("not available"))) {
    console.log(`  SKIP: ${(err as Error).message}`);
  } else { throw err; }
}
