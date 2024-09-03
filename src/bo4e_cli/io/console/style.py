import re
from typing import AnyStr, Text

from rich.color import Color
from rich.default_styles import DEFAULT_STYLES
from rich.highlighter import Highlighter, RegexHighlighter
from rich.style import Style
from rich.theme import Theme


class BO4EHighlighter(RegexHighlighter):
    base_style = "bo4e."
    highlights: list[str | re.Pattern[AnyStr]] = [
        re.compile(r"\b(?P<bo4e_bo>BO)(?P<bo4e_4e>4E)\b", re.IGNORECASE),
        re.compile(r"\b(?:(?P<bo>bo)|(?P<com>com)|(?P<enum>enum))\b", re.IGNORECASE),
        re.compile(r"(?P<version>v?\d{6}\.\d+\.\d+(?:-rc\d*)?(?:\+dev\w+)?)"),
        re.compile(r"(?P<win_path>\b[a-zA-Z]:(?:\\[-\w._+]+)*\\)(?P<filename>[-\w._+]*)"),
        re.compile(r"\b(?P<json>JSON)\b", re.IGNORECASE),
    ]


class HighlighterMixer(Highlighter):
    def __init__(self, *highlighters: Highlighter):
        self.highlighters = highlighters

    def highlight(self, text: Text) -> None:
        """Highlight :class:`rich.text.Text` using regular expressions.

        Args:
            text (~Text): Text to highlight.

        """
        for highlighter in self.highlighters:
            highlighter.highlight(text)


class ColorPalette:
    MAIN = Color.parse("#8cc04d")
    SUB = Color.parse("#617d8b")
    ERROR = Color.parse("#e35b3a")

    BO = Color.parse("#b6d7a8")
    COM = Color.parse("#e0a86c")
    ENUM = Color.parse("#d1c358")

    MAIN_ACCENT = Color.parse("#b9ff66")
    SUB_ACCENT = Color.parse("#96c1d7")
    # MAGENTA = Color.parse("#dd695f")


STYLES = {
    **DEFAULT_STYLES,
    "bo4e.bo4e_bo": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.bo4e_4e": Style(color=ColorPalette.SUB, bold=True),
    "bo4e.bo": Style(color=ColorPalette.BO, bold=True),
    "bo4e.com": Style(color=ColorPalette.COM, bold=True),
    "bo4e.enum": Style(color=ColorPalette.ENUM, bold=True),
    "bo4e.version": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.win_path": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.filename": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.json": Style(color=ColorPalette.COM),
    # These are style keys from the rich library
    "repr.ellipsis": Style(color=ColorPalette.ENUM),
    "repr.indent": Style(color=ColorPalette.MAIN, dim=True),
    "repr.error": Style(color=ColorPalette.ERROR, bold=True),
    "repr.str": Style(color=ColorPalette.MAIN, italic=False, bold=False),
    "repr.brace": Style(bold=True),
    "repr.comma": Style(bold=True),
    "repr.ipv4": Style(bold=True, color=ColorPalette.MAIN),
    "repr.ipv6": Style(bold=True, color=ColorPalette.MAIN),
    "repr.eui48": Style(bold=True, color=ColorPalette.MAIN),
    "repr.eui64": Style(bold=True, color=ColorPalette.MAIN),
    "repr.tag_start": Style(bold=True),
    "repr.tag_name": Style(color=ColorPalette.SUB, bold=True),
    "repr.tag_contents": Style(color="default"),
    "repr.tag_end": Style(bold=True),
    "repr.attrib_name": Style(color=ColorPalette.SUB, italic=False),
    "repr.attrib_equal": Style(bold=True),
    "repr.attrib_value": Style(color=ColorPalette.MAIN, italic=False),
    "repr.number": Style(color=ColorPalette.SUB_ACCENT, bold=True, italic=False),
    "repr.number_complex": Style(color=ColorPalette.SUB_ACCENT, bold=True, italic=False),  # same
    "repr.bool_true": Style(color=ColorPalette.MAIN_ACCENT, italic=True),
    "repr.bool_false": Style(color=ColorPalette.ERROR, italic=True),
    "repr.none": Style(color=ColorPalette.COM, italic=True),
    "repr.url": Style(underline=True, color=ColorPalette.MAIN, italic=False, bold=False),
    "repr.uuid": Style(color=ColorPalette.MAIN, bold=False),
    "repr.call": Style(color=ColorPalette.COM, bold=True),
    "repr.path": Style(color=ColorPalette.MAIN, bold=True),
    "repr.filename": Style(color=ColorPalette.MAIN, bold=True),
    "rule.line": Style(color=ColorPalette.SUB),
    "rule.text": Style(color=ColorPalette.MAIN),
    "bar.complete": Style(color=ColorPalette.ERROR),
    "bar.finished": Style(color=ColorPalette.MAIN),
    "bar.pulse": Style(color=ColorPalette.ERROR),
    "status.spinner": Style(color=ColorPalette.MAIN),
    "progress.description": Style.null(),
    "progress.filesize": Style(color=ColorPalette.MAIN),
    "progress.filesize.total": Style(color=ColorPalette.MAIN),
    "progress.download": Style(color=ColorPalette.MAIN),
    "progress.elapsed": Style(color=ColorPalette.ENUM, dim=True),
    "progress.percentage": Style(color=ColorPalette.SUB, bold=True),
    "progress.remaining": Style(color=ColorPalette.SUB, bold=True),
    "progress.data.speed": Style(color=ColorPalette.ERROR),
    "progress.spinner": Style(color=ColorPalette.MAIN),
    "json.brace": Style(bold=True),
    "json.bool_true": Style(color=ColorPalette.MAIN_ACCENT, bold=True),
    "json.bool_false": Style(color=ColorPalette.ERROR, bold=True),
    "json.null": Style(color=ColorPalette.COM, bold=True),
    "json.number": Style(color=ColorPalette.SUB_ACCENT, bold=True, italic=False),
    "json.str": Style(color=ColorPalette.MAIN, italic=False, bold=False),
    "json.key": Style(color=ColorPalette.SUB, bold=True),
    # These are style keys from the typer library
    "option": Style(color=ColorPalette.SUB, bold=True),
    "switch": Style(color=ColorPalette.MAIN, bold=True),
    "negative_option": Style(color=ColorPalette.COM, bold=True),
    "negative_switch": Style(color=ColorPalette.ERROR, bold=True),
    "metavar": Style(color=ColorPalette.ENUM, bold=True),
    "metavar_sep": Style(dim=True),
    "usage": Style(color=ColorPalette.ENUM),
}

BO4ETheme = Theme(STYLES)
