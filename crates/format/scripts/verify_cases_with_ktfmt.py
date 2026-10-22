#!/usr/bin/env python3
"""
Validate format crate test cases against reference ktfmt.

Discovers test fixtures from `crates/format/tests/ktfmt/cases`, parses
directives, formats each case using ktfmt, and prints any cases where ktfmt
disagrees with the expected output.
"""

import argparse
import atexit
import difflib
import glob
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import List, Optional, Tuple

CYAN = "\033[96m"
GREEN = "\033[92m"
YELLOW = "\033[93m"
RED = "\033[91m"
BOLD = "\033[1m"
RESET = "\033[0m"


def colorize(text: str, color: str, enable: bool = True) -> str:
    return f"{color}{text}{RESET}" if enable else text


JAVA_BRIDGE_SOURCE = """
import java.io.InputStreamReader;
import java.io.BufferedReader;
import java.nio.charset.StandardCharsets;
import org.jetbrains.kotlinx.ktfmt.format.Formatter;
import org.jetbrains.kotlinx.ktfmt.format.FormattingOptions;
import org.jetbrains.kotlinx.ktfmt.format.TrailingCommaManagementStrategy;
import org.jetbrains.kotlinx.ktfmt.format.FileType;
import org.jetbrains.kotlinx.ktfmt.format.KotlinCode;

public class KtfmtRunnerBridge {
    public static void main(String[] args) throws Exception {
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        String line;
        while ((line = reader.readLine()) != null) {
            if (line.equals("EXIT")) break;
            String[] parts = line.split("\\\\|", -1);
            String preset = parts[0];
            String maxWidthStr = parts[1];
            String blockIndentStr = parts[2];
            String contIndentStr = parts[3];
            String commaStrat = parts[4];
            String removeUnusedStr = parts[5];
            String preserveLambdaStr = parts[6];
            String fileTypeStr = parts[7];
            boolean experimentalEngine = Boolean.parseBoolean(parts[8]);
            int length = Integer.parseInt(parts[9]);

            char[] cbuf = new char[length];
            int read = 0;
            while (read < length) {
                int r = reader.read(cbuf, read, length - read);
                if (r == -1) break;
                read += r;
            }
            String inputCode = new String(cbuf, 0, read);

            FormattingOptions base;
            if ("google".equals(preset)) {
                base = Formatter.GOOGLE_FORMAT;
            } else if ("kotlinlang".equals(preset)) {
                base = Formatter.KOTLINLANG_FORMAT;
            } else {
                base = Formatter.META_FORMAT;
            }

            FormattingOptions.Builder b = base.toBuilder();
            if (!maxWidthStr.isEmpty()) b.maxWidth(Integer.parseInt(maxWidthStr));
            if (!blockIndentStr.isEmpty()) b.blockIndent(Integer.parseInt(blockIndentStr));
            if (!contIndentStr.isEmpty()) b.continuationIndent(Integer.parseInt(contIndentStr));
            if (!commaStrat.isEmpty()) {
                b.trailingCommaManagementStrategy(TrailingCommaManagementStrategy.valueOf(commaStrat));
            }
            if (!removeUnusedStr.isEmpty()) b.removeUnusedImports(Boolean.parseBoolean(removeUnusedStr));
            if (!preserveLambdaStr.isEmpty()) b.preserveLambdaBreaks(Boolean.parseBoolean(preserveLambdaStr));
            if (experimentalEngine) {
                b.useExperimentalEngine$org_jetbrains_kotlinx_ktfmt(true);
            }
            FormattingOptions options = b.build();

            FileType fileType = "SCRIPT".equals(fileTypeStr) ? FileType.SCRIPT : FileType.REGULAR;
            KotlinCode kc = KotlinCode.Companion.from(inputCode, fileType);

            try {
                String result = Formatter.format(options, kc);
                byte[] bytes = result.getBytes(StandardCharsets.UTF_8);
                System.out.println("OK|" + bytes.length);
                System.out.write(bytes);
                System.out.flush();
            } catch (Throwable t) {
                String err = t.getMessage() != null ? t.getMessage() : t.toString();
                byte[] bytes = err.getBytes(StandardCharsets.UTF_8);
                System.out.println("ERR|" + bytes.length);
                System.out.write(bytes);
                System.out.flush();
            }
        }
    }
}
"""


class CaseConfig:
    def __init__(
        self,
        preset: str = "meta",
        file_type: str = "REGULAR",
        max_width: Optional[int] = None,
        block_indent: Optional[int] = None,
        continuation_indent: Optional[int] = None,
        trailing_comma_strategy: str = "",
        remove_unused_imports: Optional[bool] = None,
        preserve_lambda_breaks: Optional[bool] = None,
    ):
        self.preset = preset
        self.file_type = file_type
        self.max_width = max_width
        self.block_indent = block_indent
        self.continuation_indent = continuation_indent
        self.trailing_comma_strategy = trailing_comma_strategy
        self.remove_unused_imports = remove_unused_imports
        self.preserve_lambda_breaks = preserve_lambda_breaks

    @classmethod
    def parse(cls, content: str, rel_path: str, base_preset: Optional[str] = None) -> "CaseConfig":
        if base_preset is not None:
            preset = base_preset
        elif rel_path.startswith("google"):
            preset = "google"
        elif rel_path.startswith("kotlinlang"):
            preset = "kotlinlang"
        else:
            preset = "meta"

        file_type = "SCRIPT" if content.startswith("#!") else "REGULAR"
        max_width = None
        block_indent = None
        continuation_indent = None
        trailing_comma_strategy = ""
        remove_unused_imports = None
        preserve_lambda_breaks = None

        for line in content.splitlines():
            line = line.strip()
            if not line.startswith("//"):
                break
            parts = line[2:].strip().split()
            if not parts:
                continue
            directive = parts[0]
            val = parts[1] if len(parts) > 1 else ""

            if directive == "FILE_TYPE":
                file_type = val
            elif directive == "MAX_WIDTH":
                max_width = int(val)
            elif directive == "BLOCK_INDENT":
                block_indent = int(val)
            elif directive == "CONTINUATION_INDENT":
                continuation_indent = int(val)
            elif directive == "TRAILING_COMMA_STRATEGY":
                trailing_comma_strategy = val
            elif directive == "REMOVE_UNUSED_IMPORTS":
                remove_unused_imports = val.lower() == "true"
            elif directive == "PRESERVE_LAMBDA_BREAKS":
                preserve_lambda_breaks = val.lower() == "true"

        return cls(
            preset=preset,
            file_type=file_type,
            max_width=max_width,
            block_indent=block_indent,
            continuation_indent=continuation_indent,
            trailing_comma_strategy=trailing_comma_strategy,
            remove_unused_imports=remove_unused_imports,
            preserve_lambda_breaks=preserve_lambda_breaks,
        )


def find_ktfmt_jar() -> Optional[str]:
    env_jar = os.environ.get("KTFMT_JAR")
    if env_jar and os.path.exists(env_jar):
        return env_jar

    for pattern in [
        "/opt/homebrew/Cellar/ktfmt/*/libexec/ktfmt-*-with-dependencies.jar",
        "/usr/local/Cellar/ktfmt/*/libexec/ktfmt-*-with-dependencies.jar",
    ]:
        matches = sorted(glob.glob(pattern), reverse=True)
        if matches:
            return matches[0]

    ktfmt_bin = shutil.which("ktfmt")
    if ktfmt_bin:
        try:
            with open(ktfmt_bin, "r", encoding="utf-8", errors="ignore") as fp:
                match = re.search(r'["\']([^"\']+\.jar)["\']', fp.read())
                if match and os.path.exists(match.group(1)):
                    return match.group(1)
        except Exception:
            pass

    return None


def find_java_executable() -> str:
    env_java_home = os.environ.get("JAVA_HOME")
    if env_java_home:
        candidate = os.path.join(env_java_home, "bin", "java")
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate

    homebrew_java17 = "/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home/bin/java"
    if os.path.isfile(homebrew_java17) and os.access(homebrew_java17, os.X_OK):
        return homebrew_java17

    return shutil.which("java") or "java"


class KtfmtRunner:
    def __init__(self, jar_path: Optional[str] = None):
        self.jar_path = jar_path or find_ktfmt_jar()
        self.process: Optional[subprocess.Popen] = None
        self.temp_file: Optional[str] = None

        if self.jar_path:
            self._start_java_bridge()
        else:
            self.ktfmt_bin = shutil.which("ktfmt")
            if not self.ktfmt_bin:
                raise RuntimeError("Neither ktfmt jar nor 'ktfmt' command found in PATH")

    def _start_java_bridge(self):
        fd, temp_path = tempfile.mkstemp(prefix="KtfmtRunnerBridge_", suffix=".java")
        os.close(fd)
        with open(temp_path, "w", encoding="utf-8") as fp:
            fp.write(JAVA_BRIDGE_SOURCE)
        self.temp_file = temp_path

        atexit.register(self.cleanup)

        cmd = [find_java_executable(), "-cp", self.jar_path, self.temp_file]
        self.process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def format(
        self,
        code: str,
        config: CaseConfig,
        experimental_engine: bool = False,
    ) -> Tuple[bool, str]:
        if self.process:
            max_w = str(config.max_width) if config.max_width is not None else ""
            block_i = str(config.block_indent) if config.block_indent is not None else ""
            cont_i = str(config.continuation_indent) if config.continuation_indent is not None else ""
            rem_u = str(config.remove_unused_imports).lower() if config.remove_unused_imports is not None else ""
            pres_l = str(config.preserve_lambda_breaks).lower() if config.preserve_lambda_breaks is not None else ""
            exp_eng = str(experimental_engine).lower()

            code_bytes = code.encode("utf-8")
            header = f"{config.preset}|{max_w}|{block_i}|{cont_i}|{config.trailing_comma_strategy}|{rem_u}|{pres_l}|{config.file_type}|{exp_eng}|{len(code_bytes)}\n"

            try:
                self.process.stdin.write(header.encode("utf-8") + code_bytes)
                self.process.stdin.flush()

                res_line = self.process.stdout.readline().decode("utf-8")
                if not res_line:
                    stderr = self.process.stderr.read().decode("utf-8", errors="ignore")
                    return False, f"Runner crashed: {stderr}"

                status, length_str = res_line.strip().split("|")
                res_bytes = self.process.stdout.read(int(length_str))
                return (status == "OK"), res_bytes.decode("utf-8")
            except Exception as e:
                return False, str(e)

        # Fallback to CLI
        cmd = [self.ktfmt_bin, f"--{config.preset}-style"]
        if config.remove_unused_imports is False:
            cmd.append("--do-not-remove-unused-imports")
        cmd.append("-")

        p = subprocess.run(cmd, input=code.encode("utf-8"), capture_output=True)
        if p.returncode != 0:
            return False, p.stderr.decode("utf-8", errors="ignore")
        return True, p.stdout.decode("utf-8", errors="ignore")

    def cleanup(self):
        if self.process:
            try:
                self.process.stdin.write(b"EXIT\n")
                self.process.stdin.flush()
                self.process.wait(timeout=1)
            except Exception:
                try:
                    self.process.kill()
                except Exception:
                    pass
            self.process = None

        if self.temp_file and os.path.exists(self.temp_file):
            try:
                os.remove(self.temp_file)
            except Exception:
                pass
            self.temp_file = None


def run_case_validation(cases_dir: Path, filter_str: Optional[str] = None):
    runner = KtfmtRunner()
    fixtures = sorted(cases_dir.glob("**/*.input"))
    if filter_str:
        fixtures = [p for p in fixtures if filter_str in str(p.relative_to(cases_dir))]

    return runner, fixtures


def main():
    parser = argparse.ArgumentParser(
        description="Validate format crate test cases with ktfmt and report disagreements."
    )
    parser.add_argument(
        "filter",
        nargs="?",
        default=None,
        help="Optional case name / path substring filter (e.g. 'kotlinlang', 'annotation')",
    )
    default_cases_dir = str(Path(__file__).resolve().parent.parent / "tests" / "ktfmt" / "cases")
    parser.add_argument(
        "--cases-dir",
        default=default_cases_dir,
        help=f"Path to case fixtures directory (default: {default_cases_dir})",
    )
    parser.add_argument(
        "-d",
        "--diff",
        action="store_true",
        help="Print unified diff for each disagreement",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help="Print progress for each test case",
    )
    args = parser.parse_args()

    use_color = sys.stdout.isatty()
    cases_dir = Path(args.cases_dir).resolve()
    if not cases_dir.is_dir():
        print(colorize(f"Error: Cases directory '{cases_dir}' not found.", RED, use_color))
        sys.exit(1)

    try:
        runner, fixtures = run_case_validation(cases_dir, args.filter)
    except Exception as e:
        print(colorize(f"Error: {e}", RED, use_color))
        sys.exit(1)

    if not fixtures:
        print(colorize(f"No fixtures found matching filter '{args.filter}'.", YELLOW, use_color))
        sys.exit(0)

    print(colorize(f"Discovered {len(fixtures)} test fixture(s) in {cases_dir}", BOLD, use_color))
    start_time = time.time()

    total_evaluated = 0
    agreed_count = 0
    disagreements = []
    errors = []

    for fixture in fixtures:
        rel = str(fixture.relative_to(cases_dir))
        content = fixture.read_text(encoding="utf-8")
        config = CaseConfig.parse(content, rel)

        output_path = fixture.with_suffix(".output")
        expected = output_path.read_text(encoding="utf-8") if output_path.exists() else content

        total_evaluated += 1
        ok, actual = runner.format(content, config, experimental_engine=False)
        if not ok:
            errors.append((rel, actual))
            if args.verbose:
                print(f"[{colorize('ERR', RED, use_color)}] {rel}: {actual.strip()[:80]}")
            continue

        if actual == expected:
            agreed_count += 1
            if args.verbose:
                print(f"[{colorize('AGREE', GREEN, use_color)}] {rel}")
        else:
            disagreements.append((rel, expected, actual))
            if args.verbose:
                print(f"[{colorize('DISAGREE', RED, use_color)}] {rel}")

        # Variant pass (*.new.output)
        variant_path = fixture.with_name(f"{fixture.stem}.new.output")
        if variant_path.exists():
            var_rel = f"{rel} [new]"
            var_expected = variant_path.read_text(encoding="utf-8")
            var_config = CaseConfig.parse(content, rel, base_preset="kotlinlang")
            total_evaluated += 1
            var_ok, var_actual = runner.format(content, var_config, experimental_engine=True)
            if var_ok:
                if var_actual == var_expected:
                    agreed_count += 1
                    if args.verbose:
                        print(f"[{colorize('AGREE', GREEN, use_color)}] {var_rel}")
                else:
                    disagreements.append((var_rel, var_expected, var_actual))
                    if args.verbose:
                        print(f"[{colorize('DISAGREE', RED, use_color)}] {var_rel}")

    elapsed = time.time() - start_time

    print("\n" + colorize("=" * 60, BOLD, use_color))
    print(colorize(" KTFMT CASE RUNNER SUMMARY", BOLD, use_color))
    print(colorize("=" * 60, BOLD, use_color))
    print(f"Total fixtures evaluated: {total_evaluated} in {elapsed:.2f}s")
    print(f"Agreed with ktfmt:        {colorize(str(agreed_count), GREEN, use_color)}")
    print(f"Disagreed with ktfmt:     {colorize(str(len(disagreements)), RED if disagreements else GREEN, use_color)}")
    if errors:
        print(f"Errors / crashes:         {colorize(str(len(errors)), RED, use_color)}")

    if disagreements:
        print(f"\n{colorize(f'Disagreements ({len(disagreements)} cases):', BOLD, use_color)}")
        for label, _, _ in disagreements:
            print(f"  {colorize('✗', RED, use_color)} {label}")

        if args.diff:
            print("\n" + colorize("=" * 60, BOLD, use_color))
            print(colorize(" UNIFIED DIFFS (Expected vs ktfmt)", BOLD, use_color))
            print(colorize("=" * 60, BOLD, use_color))
            for label, expected, actual in disagreements:
                print(colorize(f"\n--- {label} ---", CYAN, use_color))
                diff = difflib.unified_diff(
                    expected.splitlines(keepends=True),
                    actual.splitlines(keepends=True),
                    fromfile="expected",
                    tofile="ktfmt",
                )
                for line in diff:
                    line_s = line.rstrip("\n")
                    if line.startswith("+"):
                        print(colorize(line_s, GREEN, use_color))
                    elif line.startswith("-"):
                        print(colorize(line_s, RED, use_color))
                    elif line.startswith("@"):
                        print(colorize(line_s, CYAN, use_color))
                    else:
                        print(line_s)

    if errors:
        print(f"\n{colorize('ktfmt Errors:', RED, use_color)}")
        for label, err in errors:
            print(f"  {colorize('!', RED, use_color)} {label}: {err.strip()[:100]}")

    runner.cleanup()
    sys.exit(1 if disagreements or errors else 0)


if __name__ == "__main__":
    main()
