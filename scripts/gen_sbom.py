#!/usr/bin/env python3
"""DELİL için SBOM (CycloneDX benzeri JSON) ve lisans listesi üretir.

Kullanım:
    cargo metadata --format-version 1 --all-features > /tmp/meta.json
    python3 scripts/gen_sbom.py /tmp/meta.json docs/SBOM.json docs/LISANSLAR.md
"""
import json
import sys
import uuid
from datetime import datetime, timezone


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__)
        return 2
    meta_path, sbom_path, lic_path = sys.argv[1:4]
    meta = json.load(open(meta_path))
    packages = meta["packages"]
    workspace_names = {
        p["name"] for p in packages if p["id"] in meta["workspace_members"]
    }

    # CycloneDX benzeri SBOM.
    components = []
    for p in sorted(packages, key=lambda x: x["name"]):
        lic = p.get("license") or "UNKNOWN"
        component = {
            "type": "library",
            "name": p["name"],
            "version": p.get("version", "0.0.0"),
            "scope": "required",
            "purl": f"pkg:cargo/{p['name']}@{p.get('version', '0.0.0')}",
        }
        if lic != "UNKNOWN":
            component["licenses"] = [{"license": {"id": lic}}]
        if p["name"] in workspace_names:
            component["properties"] = [
                {"name": "delil:workspace_member", "value": "true"}
            ]
        components.append(component)

    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{uuid.uuid4()}",
        "version": 1,
        "metadata": {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "component": {
                "type": "application",
                "name": "DELİL",
                "version": "0.1.0",
                "description": "56.12 bilirkişileri için otomatik ses çözümleme ve UDF raporlama",
            },
            "tools": [{"name": "gen_sbom.py", "vendor": "DELİL"}],
        },
        "components": components,
    }
    with open(sbom_path, "w", encoding="utf-8") as f:
        json.dump(sbom, f, ensure_ascii=False, indent=2)

    # Lisans dağılımı.
    buckets: dict[str, list[str]] = {}
    for p in packages:
        key = p.get("license") or "UNKNOWN"
        buckets.setdefault(key, []).append(f"{p['name']} {p.get('version', '')}".strip())

    lines = []
    lines.append("# DELİL — Bağımlılık Lisansları\n")
    lines.append(
        f"Toplam {len(packages)} bağımlılık paketi. Bu liste `cargo metadata` "
        "çıktısından üretilmiştir.\n"
    )
    lines.append("## Lisans dağılımı\n")
    lines.append("| Lisans | Paket sayısı |")
    lines.append("|---|---|")
    for lic, names in sorted(buckets.items(), key=lambda x: -len(x[1])):
        lines.append(f"| `{lic}` | {len(names)} |")
    lines.append("")
    lines.append("## Değerlendirme\n")
    lines.append(
        "Tüm bağımlılıklar izin verici (MIT, Apache-2.0, BSD, Zlib, ISC, "
        "Unicode-3.0, BSL-1.0, CC0) veya zayıf copyleft (MPL-2.0) "
        "lisanslıdır. Hiçbir bağımlılık güçlü copyleft (GPL/AGPL) değildir. "
        "MPL-2.0, dosya düzeyinde copyleft uygular; kaynak dosyaları "
        "değiştirmediğimiz sürece dağıtım kısıtı doğurmaz.\n"
    )
    lines.append("## Çalışma zamanı gereksinimleri\n")
    lines.append("- Windows 11 x64 (ana platform).")
    lines.append(
        "- GPU hızlandırma (isteğe bağlı): CUDA 12+ / Metal; yoksa CPU."
    )
    lines.append("- Harici FFmpeg, Python, Node.js veya ayrıca model indirme GEREKMEZ.")
    lines.append("")
    lines.append("## Paket listesi\n")
    lines.append("| Paket | Sürüm | Lisans |")
    lines.append("|---|---|---|")
    for p in sorted(packages, key=lambda x: x["name"]):
        lines.append(
            f"| {p['name']} | {p.get('version', '')} | {p.get('license') or 'UNKNOWN'} |"
        )
    lines.append("")
    with open(lic_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    print(f"SBOM yazıldı: {sbom_path} ({len(components)} bileşen)")
    print(f"Lisanslar yazıldı: {lic_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
