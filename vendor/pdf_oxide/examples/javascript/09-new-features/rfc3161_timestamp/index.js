// RFC 3161 Timestamp parsing + TsaClient construction
// Run: node index.js

import { Timestamp, TsaClient, TimestampHashAlgorithm } from "pdf-oxide";

// ── 1. Timestamp parsing ─────────────────────────────────────────────────────
console.log("Parsing RFC 3161 timestamp...");
const bareTstInfo = Buffer.from(
  "3081B302010106042A0304013031300D060960864801650304020105000420" +
  "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD" +
  "020104180F32303233303630373131323632365A300A020101800201F4810164" +
  "0101FF0208314CFCE4E0651827A048A4463044310B30090603550406130255533113" +
  "301106035504080C0A536F6D652D5374617465310D300B060355040A0C04546573" +
  "743111300F06035504030C085465737420545341",
  "hex"
);

try {
  const ts = Timestamp.parse(bareTstInfo);
  console.log(`  Time (epoch): ${ts.time}`);
  console.log(`  Serial: ${ts.serial}  Policy OID: ${ts.policyOid}`);
  if (ts.serial !== "04") throw new Error(`unexpected serial: ${ts.serial}`);
  if (ts.policyOid !== "1.2.3.4.1") throw new Error(`unexpected policy OID: ${ts.policyOid}`);
  console.log("  Timestamp fields verified.");
  // verify() requires a CMS-wrapped token; bare TSTInfo returns an error
  try {
    const result = ts.verify();
    console.log(`  verify() → ${result}`);
  } catch (verifyErr) {
    console.log(`  verify() on bare TSTInfo → error (expected): ${String(verifyErr).slice(0, 60)}`);
  }
  ts.close();
} catch (err) {
  if (err instanceof Error && (err.message.includes("not available") || err.message.includes("error code 8"))) {
    console.log("  SKIP: signatures feature not compiled in.");
  } else { throw err; }
}

// ── 2. TsaClient construction ─────────────────────────────────────────────────
console.log("Constructing TsaClient (offline, no network call)...");
try {
  const client = new TsaClient({
    url: "https://freetsa.org/tsr",
    timeoutSeconds: 30,
    hashAlgorithm: TimestampHashAlgorithm.Sha256,
    useNonce: true,
    certReq: true,
  });
  console.log("  TsaClient created (no network call).");
  client.close();
} catch (err) {
  if (err instanceof Error && (err.message.includes("not available") || err.message.includes("error code 8"))) {
    console.log("  SKIP: signatures feature not compiled in.");
  } else { throw err; }
}
process.exit(0);
