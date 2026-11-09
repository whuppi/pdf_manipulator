// RFC 3161 Timestamp parsing + TsaClient construction
// Run: go run main.go

package main

import (
	"fmt"
	"log"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

func main() {
	// ── 1. Timestamp parsing ─────────────────────────────────────────────────
	fmt.Println("Parsing RFC 3161 timestamp...")

	bareTstInfo := mustHex(
		"3081B302010106042A0304013031300D060960864801650304020105000420" +
			"BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD" +
			"020104180F32303233303630373131323632365A300A020101800201F4810164" +
			"0101FF0208314CFCE4E0651827A048A4463044310B30090603550406130255533113" +
			"301106035504080C0A536F6D652D5374617465310D300B060355040A0C04546573" +
			"743111300F06035504030C085465737420545341",
	)

	ts, err := pdfoxide.ParseTimestamp(bareTstInfo)
	if err != nil {
		fmt.Printf("  SKIP: signatures feature not available (%v)\n", err)
		return
	}

	epochSec, _ := ts.Time()
	serial, _   := ts.Serial()
	policyOid, _ := ts.PolicyOid()
	tsaName, _  := ts.TsaName()

	fmt.Printf("  Time (epoch): %d\n", epochSec)
	fmt.Printf("  Serial: %s  Policy OID: %s\n", serial, policyOid)
	fmt.Printf("  TSA name: %s\n", tsaName)
	if serial != "04" {
		log.Fatalf("unexpected serial: %s", serial)
	}
	fmt.Println("  Timestamp fields verified.")

	// ── 2. TsaClient construction ────────────────────────────────────────────
	fmt.Println("Constructing TsaClient (offline, no network call)...")
	opts := pdfoxide.TsaClientOptions{
		URL:            "https://freetsa.org/tsr",
		TimeoutSeconds: 30,
		HashAlgorithm:  pdfoxide.TimestampHashSha256,
		UseNonce:       true,
		CertReq:        true,
	}
	client, err := pdfoxide.NewTsaClient(opts)
	if err != nil {
		fmt.Printf("  SKIP: signatures feature not available (%v)\n", err)
		return
	}
	defer client.Close()
	fmt.Println("  TsaClient created (no network call).")
}

func mustHex(s string) []byte {
	b := make([]byte, 0, len(s)/2)
	for i := 0; i < len(s)-1; i += 2 {
		var v byte
		fmt.Sscanf(s[i:i+2], "%02x", &v)
		b = append(b, v)
	}
	return b
}
