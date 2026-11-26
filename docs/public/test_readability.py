"""TC-0047 LANG06: teach from focused tested regions, not test harnesses."""
import html
import json
import re
import unittest
from pathlib import Path

import build
import highlight
from source_checks import Code


class Readability(unittest.TestCase):
    def test_rust_command_guide_explains_the_todo_contract(self):
        body = (Path(__file__).parent/"pages/commands.html").read_text()
        for term in ["create-task-v1", "complete-task-v1", "scope_args", "CommandResult", "task_id", "abort"]:
            with self.subTest(concept=term):
                self.assertIn(term, body)
        self.assertNotIn("This version takes no arguments", body)

    def test_snippets_remove_only_common_outer_indentation(self):
        source = "fn main() {\n    // docs:start demo\n    let row = fetch();\n    if ready {\n        show(row);\n    }\n    // docs:end demo\n}\n"
        rendered = build.render_examples("{{example:demo:demo}}", {"demo": ("demo.rs", source)})
        code = re.search(r"<code[^>]*>(.*?)</code>", rendered, re.S)[1]
        plain = html.unescape(re.sub(r"<[^>]+>", "", code))
        self.assertEqual(plain, "let row = fetch();\nif ready {\n    show(row);\n}\n")

    def test_guide_examples_use_named_steps_without_verification_assertions(self):
        root = Path(__file__).parent
        examples = {p.stem: p for p in (build.REPO/"examples/src/bin").glob("*.rs")}
        examples.update({name:build.REPO/"examples"/entry["path"] for name,entry in json.loads((root/"language-examples.json").read_text()).items()})
        examples.update({"support":build.REPO/"examples/src/lib.rs", "application":build.REPO/"examples/application.mjs", "javascript":build.REPO/"examples/javascript.mjs"})
        for page in (root/"pages").glob("*.html"):
            body = page.read_text()
            for include in re.finditer(r"\{\{example:([\w-]+)(?::([\w-]+))?\}\}", body):
                name, region = include.groups()
                with self.subTest(page=page.name, example=name, region=region):
                    self.assertIsNotNone(region, "guide code must teach one named step; link the full file separately")
                    if region is None: continue
                    snippet = build.example_regions(examples[name].read_text())[region]
                    self.assertNotRegex(snippet, r"\bassert(?:_eq|_ne)?!|\b(?:assert|check)\s*\(", "verification checks belong in the runnable file, outside tutorial snippets")
                    code = Code(highlight.highlight(snippet, highlight.language(examples[name]))).source
                    brackets = []
                    for char in code:
                        if char in "([{": brackets.append(char)
                        elif char in ")]}":
                            self.assertTrue(brackets, "snippet must start at a whole statement")
                            self.assertEqual(brackets.pop(), {")":"(","]":"[","}":"{"}[char])
                    self.assertEqual(brackets, [], "snippet must finish its statements")
                    before = body[:include.start()]
                    self.assertTrue(re.search(r"</(?:p|ul|ol)>\s*$", before), "introduce each snippet with explanatory prose")


if __name__ == "__main__": unittest.main()
