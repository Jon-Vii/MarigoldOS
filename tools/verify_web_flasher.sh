#!/usr/bin/env bash
set -euo pipefail

site_dir=${1:?usage: tools/verify_web_flasher.sh SITE_DIR [--safety-only]}

# Safety contract: these are app-only manifests (one part at the app offset),
# so the installer must never be able to erase the whole chip — that would
# wipe the bootloader and partition table with nothing in the manifest to
# restore them, leaving the device unbootable until a USB reflash.
python3 - "$site_dir" <<'PY'
import copy
import hashlib
import json
import pathlib
import re
import sys

site = pathlib.Path(sys.argv[1])

# Any change to the vendored dialog bundle must be re-reviewed against the
# erase-safety contract below and this hash updated in the same commit.
DIALOG_SHA256 = "5751da36ccefb538d349baa3b1e6b6206cf8349958f7870f99a115e8c55c8860"

# The one erasing install lives behind this exact conjunction: same-version
# reinstall, manifest opt-in, and a recovery bundle covering the bootloader,
# partition table, otadata, and app offsets.
ERASE_GATE = (
    "this._isSameVersion&&this._manifest.full_erase_allowed&&"
    "this._manifest.builds.every((e=>[0,32768,57344,65536]"
    ".every((t=>e.parts.some((e=>e.offset===t))))))?"
)


def check_manifest(name, manifest):
    if manifest.get("new_install_prompt_erase") is not False:
        raise SystemExit(f"{name}: app-only installs must not prompt for erase")
    if manifest.get("full_erase_allowed") is not False:
        raise SystemExit(f"{name}: app-only installs must forbid whole-chip erase")
    builds = manifest.get("builds") or []
    if not builds:
        raise SystemExit(f"{name}: no firmware builds")
    for build in builds:
        parts = build.get("parts") or []
        if not parts or any(part.get("offset") != 0x10000 for part in parts):
            raise SystemExit(f"{name}: app-only parts must all target 0x10000")


def check_dialog(name, dialog):
    try:
        no_improv = dialog.split("}_renderDashboardNoImprov(){", 1)[1].split(
            "_renderProvision()", 1
        )[0]
    except IndexError as error:
        raise SystemExit(f"{name}: cannot locate no-Improv install path") from error
    if "_startInstall(!1)" not in no_improv or "_startInstall(!0)" in no_improv:
        raise SystemExit(
            f"{name}: no-Improv app install is not unconditionally non-erasing"
        )

    if dialog.count("_startInstall(e){") != 1:
        raise SystemExit(f"{name}: expected exactly one _startInstall definition")
    call_sites = [m.end() for m in re.finditer(re.escape("_startInstall("), dialog)]
    non_literal = [i for i in call_sites if dialog[i : i + 3] not in ("!0)", "!1)")]
    if len(non_literal) != 1:
        raise SystemExit(
            f"{name}: every install call must pass a literal erase flag"
        )

    gate_at = dialog.find(ERASE_GATE)
    if gate_at < 0 or dialog.find(ERASE_GATE, gate_at + 1) >= 0:
        raise SystemExit(
            f"{name}: full erase must be gated exactly once on same-version +"
            " full_erase_allowed + complete recovery bundle"
        )
    gate_end = dialog.find('`:""', gate_at)
    if gate_end < 0:
        raise SystemExit(f"{name}: cannot locate the end of the gated erase entry")
    erase_calls = [
        m.start() for m in re.finditer(re.escape("_startInstall(!0)"), dialog)
    ]
    if len(erase_calls) != 1 or not gate_at < erase_calls[0] < gate_end:
        raise SystemExit(
            f"{name}: the sole erasing install must sit inside the gated erase entry"
        )


def check_dialog_hash(name, data):
    digest = hashlib.sha256(data).hexdigest()
    if digest != DIALOG_SHA256:
        raise SystemExit(
            f"{name}: vendored dialog changed (sha256 {digest}); re-review its"
            " install paths against the erase-safety contract and update"
            " DIALOG_SHA256"
        )


manifests = sorted(site.glob("manifest-x*.json"))
if not manifests:
    raise SystemExit(f"{site}: no flasher manifests found")
for manifest_path in manifests:
    check_manifest(manifest_path, json.loads(manifest_path.read_text()))

dialog_path = site / "vendor/esp-web-tools/web/install-dialog-C5LjR_e6.js"
dialog_bytes = dialog_path.read_bytes()
dialog = dialog_bytes.decode("utf-8")
check_dialog(dialog_path, dialog)
check_dialog_hash(dialog_path, dialog_bytes)


# Self-test: the checks above must reject known-dangerous mutations of the
# shipped files; a verifier that passes any of them is itself broken.
def expect_rejection(label, check, name, subject):
    try:
        check(name, subject)
    except SystemExit:
        return
    raise SystemExit(f"verifier self-test: {label} passed the safety check")


def replace_nth(text, old, new, n):
    at = -1
    for _ in range(n):
        at = text.index(old, at + 1)
    return text[: at] + new + text[at + len(old) :]


for n in range(1, dialog.count("_startInstall(!1)") + 1):
    expect_rejection(
        f"non-erasing install call {n} flipped to erasing",
        check_dialog,
        dialog_path,
        replace_nth(dialog, "_startInstall(!1)", "_startInstall(!0)", n),
    )
expect_rejection(
    "erase gate weakened from && to ||",
    check_dialog,
    dialog_path,
    dialog.replace(
        "this._isSameVersion&&this._manifest.full_erase_allowed",
        "this._isSameVersion||this._manifest.full_erase_allowed",
    ),
)
expect_rejection(
    "full_erase_allowed dropped from the erase gate",
    check_dialog,
    dialog_path,
    dialog.replace("this._manifest.full_erase_allowed&&", ""),
)
expect_rejection(
    "recovery bundle reduced to the app offset",
    check_dialog,
    dialog_path,
    dialog.replace("[0,32768,57344,65536]", "[65536]"),
)
expect_rejection(
    "second erasing install added outside the gate",
    check_dialog,
    dialog_path,
    dialog + "\n;(()=>this._startInstall(!0));",
)
expect_rejection(
    "install call with a non-literal erase flag added",
    check_dialog,
    dialog_path,
    dialog + "\n;(e=>this._startInstall(e));",
)
expect_rejection(
    "dialog bundle modified without updating DIALOG_SHA256",
    check_dialog_hash,
    dialog_path,
    dialog_bytes + b" ",
)

manifest = json.loads(manifests[0].read_text())
for label, mutate in (
    ("full_erase_allowed set to true", lambda m: m.update(full_erase_allowed=True)),
    (
        "full_erase_allowed removed",
        lambda m: m.pop("full_erase_allowed", None),
    ),
    (
        "new-install erase prompt re-enabled",
        lambda m: m.update(new_install_prompt_erase=True),
    ),
    (
        "part moved off the app offset",
        lambda m: m["builds"][0]["parts"][0].update(offset=0),
    ),
):
    mutant = copy.deepcopy(manifest)
    mutate(mutant)
    expect_rejection(label, check_manifest, manifests[0], mutant)
PY

if [[ ${2:-} == --safety-only ]]; then
  echo "web flasher is app-only and non-erasing"
  exit 0
fi

for manifest in "$site_dir"/manifest-x*.json; do
  while IFS= read -r path; do
    case "$path" in
      http://*|https://*|/*)
        echo "flasher asset must be a same-origin relative path: $path" >&2
        exit 1
        ;;
    esac
    if [[ ! -f "$site_dir/$path" ]]; then
      echo "flasher asset is missing from the published site: $path" >&2
      exit 1
    fi
  done < <(awk -F '"' '/"path"[[:space:]]*:/{print $4}' "$manifest")
done

echo "web flasher is app-only, non-erasing, same-origin, and complete"
