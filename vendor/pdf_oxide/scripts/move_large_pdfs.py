#!/usr/bin/env python3
"""Move large PDFs to separate directory for benchmarking."""

import pathlib
import shutil


# Files to move based on benchmark slowness
large_files = [
    # CFR regulation files (30 files, 0.7-2.3s each)
    "CFR_2024_Title07_Vol1_Agriculture.pdf",
    "CFR_2024_Title08_Vol1_Aliens_and_Nationality.pdf",
    "CFR_2024_Title10_Vol1_Energy.pdf",
    "CFR_2024_Title12_Vol1_Banks_and_Banking.pdf",
    "CFR_2024_Title14_Vol1_Aeronautics_and_Space.pdf",
    "CFR_2024_Title15_Vol1_Commerce_and_Foreign_Trade.pdf",
    "CFR_2024_Title16_Vol1_Commercial_Practices.pdf",
    "CFR_2024_Title17_Vol1_Commodity_and_Securities_Exchanges.pdf",
    "CFR_2024_Title18_Vol1_Conservation_of_Power_and_Water_Resources.pdf",
    "CFR_2024_Title19_Vol1_Customs_Duties.pdf",
    "CFR_2024_Title20_Vol1_Employees'_Benefits.pdf",
    "CFR_2024_Title21_Vol1_Food_and_Drugs.pdf",
    "CFR_2024_Title24_Vol1_Housing_and_Urban_Development.pdf",
    "CFR_2024_Title27_Vol1_Alcohol,_Tobacco_Products_and_Firearms.pdf",
    "CFR_2024_Title29_Vol1_Labor.pdf",
    "CFR_2024_Title30_Vol1_Mineral_Resources.pdf",
    "CFR_2024_Title33_Vol1_Navigation_and_Navigable_Waters.pdf",
    "CFR_2024_Title34_Vol1_Education.pdf",
    "CFR_2024_Title36_Vol1_Parks,_Forests,_and_Public_Property.pdf",
    "CFR_2024_Title37_Vol1_Patents,_Trademarks,_and_Copyrights.pdf",
    "CFR_2024_Title38_Vol1_Pensions,_Bonuses,_and_Veterans'_Relief.pdf",
    "CFR_2024_Title40_Vol1_Protection_of_Environment.pdf",
    "CFR_2024_Title42_Vol1_Public_Health.pdf",
    "CFR_2024_Title43_Vol1_Public_Lands:_Interior.pdf",
    "CFR_2024_Title44_Vol1_Emergency_Management_and_Assistance.pdf",
    "CFR_2024_Title45_Vol1_Public_Welfare.pdf",
    "CFR_2024_Title47_Vol1_Telecommunication.pdf",
    "CFR_2024_Title49_Vol1_Transportation.pdf",
    "CFR_2024_Title50_Vol1_Wildlife_and_Fisheries.pdf",
    # Internet Archive newspaper scans (4 files, 5-6s each)
    "IA_001-jan.-4-1940-dec.-30-1941a.pdf",
    "IA_001-jan.-4-1940-dec.-30-1941b.pdf",
    "IA_002-jan.-8-1942-dec.-29-1943a.pdf",
    "IA_002-jan.-8-1942-dec.-29-1943b.pdf",
]

pdf_dir = pathlib.Path("test_datasets/pdfs")
large_dir = pathlib.Path("test_datasets/pdf_large")
large_dir.mkdir(parents=True, exist_ok=True)

moved_count = 0
not_found = []

for filename in large_files:
    # Search for file in all subdirectories
    found_files = list(pdf_dir.rglob(filename))

    if found_files:
        for pdf_file in found_files:
            dest = large_dir / pdf_file.name
            print(f"Moving {pdf_file} -> {dest}")
            shutil.move(str(pdf_file), str(dest))
            moved_count += 1
    else:
        not_found.append(filename)

print(f"\n✓ Moved {moved_count} large PDFs to {large_dir}")

if not_found:
    print(f"\n⚠ Could not find {len(not_found)} files:")
    for filename in not_found:
        print(f"  - {filename}")

# Count remaining PDFs
remaining = list(pdf_dir.rglob("*.pdf"))
print(f"\n📊 Remaining PDFs in corpus: {len(remaining)}")
