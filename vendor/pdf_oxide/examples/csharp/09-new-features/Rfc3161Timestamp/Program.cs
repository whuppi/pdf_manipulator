// RFC 3161 Timestamp parsing + TsaClient construction
// Run: dotnet run

using System;
using PdfOxide.Core;
using PdfOxide.Exceptions;

// ── 1. Timestamp parsing ─────────────────────────────────────────────────────
Console.WriteLine("Parsing RFC 3161 timestamp...");

var bareTstInfo = Convert.FromHexString(
    "3081B302010106042A0304013031300D060960864801650304020105000420" +
    "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD" +
    "020104180F32303233303630373131323632365A300A020101800201F4810164" +
    "0101FF0208314CFCE4E0651827A048A4463044310B30090603550406130255533113" +
    "301106035504080C0A536F6D652D5374617465310D300B060355040A0C04546573" +
    "743111300F06035504030C085465737420545341");

try
{
    using var ts = Timestamp.Parse(bareTstInfo);
    Console.WriteLine($"  Time (epoch): {ts.Time.ToUnixTimeSeconds()}");
    Console.WriteLine($"  Serial: {ts.Serial}  Policy OID: {ts.PolicyOid}");
    if (ts.Serial != "04") throw new InvalidOperationException($"unexpected serial: {ts.Serial}");
    Console.WriteLine("  Timestamp fields verified.");
}
catch (UnsupportedFeatureException)
{
    Console.WriteLine("  SKIP: signatures feature not compiled in.");
}

// ── 2. TsaClient construction ─────────────────────────────────────────────────
Console.WriteLine("Constructing TsaClient (offline, no network call)...");
try
{
    using var client = TsaClient.Create(new TsaClientOptions
    {
        Url            = "https://freetsa.org/tsr",
        TimeoutSeconds = 30,
        HashAlgorithm  = TimestampHashAlgorithm.Sha256,
        UseNonce       = true,
        CertReq        = true,
    });
    Console.WriteLine($"  TsaClient created: {client}");
}
catch (UnsupportedFeatureException)
{
    Console.WriteLine("  SKIP: signatures feature not compiled in.");
}
