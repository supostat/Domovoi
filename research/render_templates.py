"""Render golden ChatML prompts for every catalog entry.

Reads assets/catalog.json and assets/chat-templates/dialogs.json, takes the chat template embedded in the
entry's GGUF file at the pinned revision (tokenizer.chat_template, read through HTTP range requests), renders
every dialog with jinja2 the way transformers' apply_chat_template does (enable_thinking=False, no tools) and
writes assets/chat-templates/<repo>@<revision>.golden.txt.

Usage: python3 research/render_templates.py
"""

import json
import struct
import urllib.request
from pathlib import Path

import jinja2
from jinja2.ext import loopcontrols
from jinja2.sandbox import ImmutableSandboxedEnvironment

ROOT = Path(__file__).resolve().parent.parent
CATALOG = ROOT / "assets" / "catalog.json"
TEMPLATES = ROOT / "assets" / "chat-templates"
DIALOGS = TEMPLATES / "dialogs.json"
DIALOG_MARKER = "<<<dialog {name}>>>\n"
RANGE_CHUNK = 8 * 1024 * 1024
GGUF_MAGIC = b"GGUF"
CHAT_TEMPLATE_KEY = "tokenizer.chat_template"
SCALAR_FORMATS = {0: "B", 1: "b", 2: "H", 3: "h", 4: "I", 5: "i", 6: "f", 7: "?", 10: "Q", 11: "q", 12: "d"}
STRING_TYPE = 8
ARRAY_TYPE = 9


class RemoteFile:
    def __init__(self, url):
        self.url = url
        self.data = b""

    def read(self, offset, length):
        while len(self.data) < offset + length:
            start = len(self.data)
            request = urllib.request.Request(self.url, headers={"Range": f"bytes={start}-{start + RANGE_CHUNK - 1}"})
            with urllib.request.urlopen(request, timeout=60) as response:
                chunk = response.read()
            if not chunk:
                raise EOFError(f"{self.url} ended before the GGUF header did")
            self.data += chunk
        return self.data[offset:offset + length]


class HeaderReader:
    def __init__(self, remote):
        self.remote = remote
        self.offset = 0

    def take(self, length):
        value = self.remote.read(self.offset, length)
        self.offset += length
        return value

    def scalar(self, fmt):
        return struct.unpack("<" + fmt, self.take(struct.calcsize("<" + fmt)))[0]

    def string(self):
        return self.take(self.scalar("Q")).decode("utf-8")

    def value(self, kind):
        if kind in SCALAR_FORMATS:
            return self.scalar(SCALAR_FORMATS[kind])
        if kind == STRING_TYPE:
            return self.string()
        if kind == ARRAY_TYPE:
            item_kind = self.scalar("I")
            return [self.value(item_kind) for _ in range(self.scalar("Q"))]
        raise ValueError(f"unknown GGUF value type {kind}")


def embedded_chat_template(entry):
    url = f"https://huggingface.co/{entry['repo']}/resolve/{entry['revision']}/{entry['file']}"
    reader = HeaderReader(RemoteFile(url))
    if reader.take(len(GGUF_MAGIC)) != GGUF_MAGIC:
        raise ValueError(f"{url} is not a GGUF file")
    reader.scalar("I")
    reader.scalar("Q")
    for _ in range(reader.scalar("Q")):
        key = reader.string()
        value = reader.value(reader.scalar("I"))
        if key == CHAT_TEMPLATE_KEY:
            return value
    raise KeyError(f"{url} carries no {CHAT_TEMPLATE_KEY}")


def raise_exception(message):
    raise jinja2.exceptions.TemplateError(message)


def tojson(value, ensure_ascii=False, indent=None, separators=None, sort_keys=False):
    return json.dumps(value, ensure_ascii=ensure_ascii, indent=indent, separators=separators, sort_keys=sort_keys)


def compile_template(source):
    environment = ImmutableSandboxedEnvironment(trim_blocks=True, lstrip_blocks=True, extensions=[loopcontrols])
    environment.filters["tojson"] = tojson
    environment.globals["raise_exception"] = raise_exception
    return environment.from_string(source)


def golden_text(template, dialogs):
    parts = []
    for dialog in dialogs:
        prompt = template.render(
            messages=dialog["messages"],
            tools=None,
            add_generation_prompt=True,
            enable_thinking=False,
        )
        parts.append(DIALOG_MARKER.format(name=dialog["name"]) + prompt)
    return "".join(parts)


def main():
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    dialogs = json.loads(DIALOGS.read_text(encoding="utf-8"))["dialogs"]
    for entry in catalog["entries"]:
        template = compile_template(embedded_chat_template(entry))
        golden = TEMPLATES / f"{entry['repo']}@{entry['revision']}.golden.txt"
        golden.parent.mkdir(parents=True, exist_ok=True)
        with golden.open("w", encoding="utf-8", newline="") as output:
            output.write(golden_text(template, dialogs))
        print(f"{entry['repo']}@{entry['revision']} -> {golden.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
