#!/usr/bin/env python3
"""Build a tiny static site from docs/*.md.

This keeps the build self-contained and avoids extra tooling.
"""
from __future__ import annotations

import argparse
import re
from pathlib import Path


def extract_title(markdown_text: str, fallback: str) -> str:
	for line in markdown_text.splitlines():
		if line.startswith("# "):
			return line[2:].strip()
	return fallback


def rewrite_md_links(markdown_text: str) -> str:
	# Replace local .md links with .html (preserve anchors).
	pattern = re.compile(r"\(([^)]+)\.md(#[^)]+)?\)")

	def repl(match: re.Match[str]) -> str:
		path = match.group(1)
		anchor = match.group(2) or ""
		return f"({path}.html{anchor})"

	return pattern.sub(repl, markdown_text)


def build_site(input_dir: Path, output_dir: Path) -> None:
	try:
		import markdown  # type: ignore
	except ImportError as exc:
		raise SystemExit(
			"Missing dependency: markdown. Install with `pip install markdown`."
		) from exc

	docs = sorted(p for p in input_dir.glob("*.md") if p.is_file())
	if not docs:
		raise SystemExit(f"No markdown files found in {input_dir}")

	output_dir.mkdir(parents=True, exist_ok=True)

	titles = {}
	contents = {}
	for path in docs:
		source = path.read_text(encoding="utf-8")
		source = rewrite_md_links(source)
		title = extract_title(source, path.stem)
		titles[path.name] = title
		contents[path.name] = markdown.markdown(
			source,
			extensions=["fenced_code", "tables", "toc"],
		)

	nav_items = []
	for name in docs:
		filename = name.name
		label = titles[filename]
		page = "index.html" if filename.lower() == "readme.md" else filename.replace(
			".md", ".html"
		)
		nav_items.append(f"<li><a href=\"{page}\">{label}</a></li>")

	nav_html = "\n".join(nav_items)
	style = """
:root {
	--bg: #0d1117;
	--fg: #e6edf3;
	--muted: #8b949e;
	--accent: #2f81f7;
	--card: #161b22;
}
* { box-sizing: border-box; }
body {
	margin: 0;
	font-family: "IBM Plex Sans", "Source Sans 3", "Segoe UI", sans-serif;
	background: radial-gradient(circle at top, #1f2937 0%, #0d1117 55%);
	color: var(--fg);
	line-height: 1.6;
}
main {
	display: grid;
	grid-template-columns: minmax(220px, 260px) minmax(0, 1fr);
	gap: 32px;
	max-width: 1100px;
	margin: 0 auto;
	padding: 40px 24px 64px;
}
nav {
	background: var(--card);
	border-radius: 16px;
	padding: 24px;
	position: sticky;
	top: 24px;
	align-self: start;
	box-shadow: 0 16px 40px rgba(0,0,0,0.35);
}
nav h2 { margin-top: 0; font-size: 18px; }
nav ul { list-style: none; padding-left: 0; margin: 0; }
nav li { margin-bottom: 10px; }
nav a { color: var(--fg); text-decoration: none; }
nav a:hover { color: var(--accent); }
article {
	background: var(--card);
	border-radius: 20px;
	padding: 32px;
	box-shadow: 0 20px 50px rgba(0,0,0,0.4);
}
article h1, article h2, article h3 { color: #f0f6fc; }
article a { color: var(--accent); }
article code { background: #0b1524; padding: 2px 6px; border-radius: 6px; }
article pre { background: #0b1524; padding: 16px; border-radius: 12px; overflow-x: auto; }
footer { color: var(--muted); margin-top: 40px; font-size: 14px; }
@media (max-width: 900px) {
	main { grid-template-columns: 1fr; }
	nav { position: relative; top: 0; }
}
"""

	def render_page(title: str, body: str) -> str:
		return f"""<!doctype html>
<html lang=\"en\">
<head>
	<meta charset=\"utf-8\" />
	<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
	<title>{title}</title>
	<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\" />
	<link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin />
	<link href=\"https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;600;700&display=swap\" rel=\"stylesheet\" />
	<style>{style}</style>
</head>
<body>
	<main>
		<nav>
			<h2>ContextDB Docs</h2>
			<ul>{nav_html}</ul>
		</nav>
		<article>
			{body}
			<footer>Built from the docs/ directory.</footer>
		</article>
	</main>
</body>
</html>"""

	for filename, body in contents.items():
		page = "index.html" if filename.lower() == "readme.md" else filename.replace(
			".md", ".html"
		)
		title = titles[filename]
		(output_dir / page).write_text(render_page(title, body), encoding="utf-8")


if __name__ == "__main__":
	parser = argparse.ArgumentParser()
	parser.add_argument("--input", default="docs", help="Input docs directory")
	parser.add_argument("--output", default="docs/site", help="Output site directory")
	args = parser.parse_args()

	build_site(Path(args.input), Path(args.output))
