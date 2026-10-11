#!/usr/bin/env python3
"""Test the production installer's extracted desktop-generation stage only.

No installer, Cargo, binary, desktop database, defaults, settings or user profile
is accessed. These are packaging tests, NOT cold/running-app activation tests.
"""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = (ROOT / "tools/install-megamail.sh").read_text(encoding="utf-8")
TEMPLATE = (ROOT / "data/megamail.desktop").read_text(encoding="utf-8")
STAGE_START = 'python3 - "$desktop_template" "$temporary_desktop" "$installed_binary" <<\'PY\'\n'
STAGE = SCRIPT.split(STAGE_START, 1)[1].split("\nPY\n", 1)[0]
PURE_START = "# BEGIN PURE_DESKTOP_EXEC_GENERATION (extracted by packaging tests)\n"
PURE_END = "# END PURE_DESKTOP_EXEC_GENERATION"
PURE = STAGE.split(PURE_START, 1)[1].split(PURE_END, 1)[0]
NAMESPACE = {}
exec(compile(PURE, "installer:quote_exec_argument", "exec"), NAMESPACE)
quote_exec_argument = NAMESPACE["quote_exec_argument"]


def desktop_string_decode(value):
    """First decode Desktop Entry string escapes, separately from Exec quoting."""
    result = []
    index = 0
    escapes = {"s": " ", "n": "\n", "t": "\t", "r": "\r", "\\": "\\"}
    while index < len(value):
        char = value[index]
        if char == "\\":
            index += 1
            if index >= len(value) or value[index] not in escapes:
                raise ValueError("invalid desktop string escape")
            char = escapes[value[index]]
        result.append(char)
        index += 1
    return "".join(result)


def quoted_exec_decode(value):
    """Decode one complete quoted Exec argument under the specification rules."""
    if not value.startswith('"') or not value.endswith('"'):
        raise ValueError("not one quoted argument")
    inner = value[1:-1]
    result = []
    index = 0
    while index < len(inner):
        char = inner[index]
        if char == "\\":
            index += 1
            if index >= len(inner) or inner[index] not in '\\`"$':
                raise ValueError("invalid Exec quote escape")
            char = inner[index]
        elif char in '`"$':
            raise ValueError("unescaped Exec reserved character")
        result.append(char)
        index += 1
    return "".join(result)


def expand_literal_percents(value):
    """The executable token may contain only literal %% codes, never %u etc."""
    output = []
    index = 0
    while index < len(value):
        if value[index] == "%":
            if value[index:index + 2] != "%%":
                raise ValueError("nonliteral field code in executable")
            index += 1
        output.append(value[index])
        index += 1
    return "".join(output)


def decode_exec(value, uri=None):
    if not value.endswith(" %u"):
        raise ValueError("URI placeholder must be a separate unquoted argument")
    executable = expand_literal_percents(quoted_exec_decode(desktop_string_decode(value[:-3])))
    # Field substitution happens only once, after tokenization. The URI is not
    # shell text, form-decoded text or another set of field codes.
    return [executable] + ([] if uri is None else [uri])


class ActivationPackagingTests(unittest.TestCase):
    def generate(self, executable, template=TEMPLATE):
        with tempfile.TemporaryDirectory(prefix="megamail-packaging-") as directory:
            source = Path(directory) / "source.desktop"
            destination = Path(directory) / "generated.desktop"
            source.write_text(template, encoding="utf-8")
            result = subprocess.run(
                [sys.executable, "-", str(source), str(destination), executable],
                input=STAGE, text=True, capture_output=True, check=False,
            )
            content = destination.read_text(encoding="utf-8") if destination.exists() else None
            mode = destination.stat().st_mode & 0o777 if destination.exists() else None
            return result, content, mode

    def test_template_exposes_one_uri_and_mailto_only(self):
        self.assertEqual([line for line in TEMPLATE.splitlines() if line.startswith("Exec=")], ["Exec=megamail %u"])
        self.assertEqual([line for line in TEMPLATE.splitlines() if line.startswith("MimeType=")], ["MimeType=x-scheme-handler/mailto;"])
        self.assertNotIn("DBusActivatable=true", TEMPLATE)

    def test_extracted_production_stage_preserves_path_escaping(self):
        paths = [
            "/home/fiction/.local/bin/megamail",
            "/home/fiction/space and Unicode 日本語/megamail",
            '/home/fiction/a"b\\c`d$e\'f;g&h|i(megamail)/100% %u %f %%/megamail',
            "/home/fiction/\\\\/literal-percent%/megamail",
        ]
        for executable in paths:
            with self.subTest(executable=executable):
                result, content, mode = self.generate(executable)
                self.assertEqual(result.returncode, 0, result.stderr)
                exec_value = next(line[5:] for line in content.splitlines() if line.startswith("Exec="))
                self.assertEqual(decode_exec(exec_value), [executable])
                self.assertEqual(mode, 0o644)
                self.assertIn("MimeType=x-scheme-handler/mailto;", content)

    def test_uri_replacement_is_one_argument_unchanged_and_not_reexpanded(self):
        exec_value = quote_exec_argument('/home/fiction/100% %u/"\\`$megamail') + " %u"
        uris = [
            "mailto:a@example.test?subject=two%20words&body=%2520+literal",
            "mailto:a@example.test?cc=c@example.test&bcc=b@example.test&body=%0D%0A",
            "MAILTO:?subject=日本語%20%25u&body=%22%24%60%5C%26",
            'mailto:?body="$(not-a-shell-command)";%u&subject=literal%f',
        ]
        for uri in uris:
            with self.subTest(uri=uri):
                self.assertEqual(decode_exec(exec_value, uri), ['/home/fiction/100% %u/"\\`$megamail', uri])

    def test_control_relative_and_equal_paths_fail_without_output(self):
        for executable in ["relative/megamail", "/a\nb", "/a\rb", "/a\tb", "/a\x00b", "/a\x7fb", "/a=b/megamail"]:
            with self.subTest(executable=repr(executable)):
                # Test the actual pure production function for embedded NUL too:
                # OS argv cannot carry NUL even to an injected generation stage.
                with self.assertRaises(ValueError):
                    quote_exec_argument(executable)
                if "\x00" not in executable:
                    result, content, _ = self.generate(executable)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIsNone(content)

    def test_bad_template_keys_fail_closed(self):
        for template in [
            TEMPLATE.replace("Exec=megamail %u\n", ""),
            TEMPLATE + "Exec=second %u\n",
            TEMPLATE.replace("MimeType=x-scheme-handler/mailto;", "MimeType=text/html;"),
            TEMPLATE + "MimeType=x-scheme-handler/mailto;\n",
        ]:
            result, content, _ = self.generate("/fiction/megamail", template)
            self.assertNotEqual(result.returncode, 0)
            self.assertIsNone(content)

    def test_reference_decoder_detects_original_single_layer_escape_bug(self):
        # The former installer escaped Exec tokens but not Desktop Entry strings.
        # A quote or literal slash must not pass the two-stage decoder by accident.
        old_value = '"/home/fiction/a\\"b" %u'
        with self.assertRaises(ValueError):
            decode_exec(old_value)
        with self.assertRaises(ValueError):
            decode_exec('"/fiction/unsafe%u" %u')


if __name__ == "__main__":
    unittest.main(verbosity=2)
