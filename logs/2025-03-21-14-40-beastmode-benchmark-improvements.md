# Task Log: Benchmark Improvements Completion

- **Actions**: Updated run.py (new terminal reporter + --html flag), pyproject.toml (7 engine extras + uv conflicts), bench.sh (sub-commands: compare/list/install/help), Makefile (4 new targets). Fixed marker/mineru pillow conflict. Ran full 3-engine comparison. Added benchmark/.gitignore.
- **Decisions**: Used [tool.uv.conflicts] to declare marker/mineru incompatibility. Removed `all-engines` group, added `compare-light` for safe lightweight installs.
- **Next steps**: Open benchmark/reports/benchmark-latest.html in browser to review HTML report visually. Consider adding docling to comparison (heavier, may need GPU).
- **Lessons/insights**: marker-pdf pins pillow<11 while mineru requires pillow>=11 — cannot coexist in same venv. uv conflicts declaration resolves lockfile generation.
