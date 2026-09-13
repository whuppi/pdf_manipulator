# RFC 3161 Timestamp parsing + TsaClient construction
#
# Run: python main.py

from __future__ import annotations

import pdf_oxide


def main() -> None:
    # ── 1. Timestamp parsing ─────────────────────────────────────────────────
    print("Parsing RFC 3161 timestamp...")
    bare_tst_info = bytes.fromhex(
        "3081B302010106042A0304013031300D060960864801650304020105000420"
        "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        "020104180F32303233303630373131323632365A300A020101800201F4810164"
        "0101FF0208314CFCE4E0651827A048A4463044310B30090603550406130255533113"
        "301106035504080C0A536F6D652D5374617465310D300B060355040A0C04546573"
        "743111300F06035504030C085465737420545341"
    )
    try:
        ts = pdf_oxide.Timestamp.parse(bare_tst_info)
        print(f"  Time (epoch): {ts.time}")
        print(f"  Serial: {ts.serial}  Policy OID: {ts.policy_oid}")
        print(f"  TSA name: {ts.tsa_name}")
        assert ts.serial == "04", f"unexpected serial: {ts.serial}"
        assert ts.policy_oid == "1.2.3.4.1"
        print("  Timestamp fields verified.")
        # verify() requires a CMS-wrapped token; bare TSTInfo returns an error
        try:
            result = ts.verify()
            print(f"  verify() → {result}")
        except RuntimeError as e:
            print(f"  verify() on bare TSTInfo → error (expected): {str(e)[:60]}")
    except (NotImplementedError, AttributeError):
        print("  SKIP: signatures feature not compiled in.")

    # ── 2. TsaClient construction ────────────────────────────────────────────
    print("Constructing TsaClient (offline, no network call)...")
    try:
        client = pdf_oxide.TsaClient(
            url="https://freetsa.org/tsr",
            timeout_seconds=30,
            hash_algorithm=2,
            use_nonce=True,
            cert_req=True,
        )
        print(f"  TsaClient created: {client!r}")
    except (NotImplementedError, AttributeError):
        print("  SKIP: signatures feature not compiled in.")


if __name__ == "__main__":
    main()
