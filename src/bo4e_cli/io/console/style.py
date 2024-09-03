"""
This module contains the style definitions and highlighters for the BO4E CLI.
"""

import re

from rich.color import Color
from rich.default_styles import DEFAULT_STYLES
from rich.highlighter import Highlighter, RegexHighlighter
from rich.style import Style
from rich.text import Text
from rich.theme import Theme

from bo4e_cli.models.meta import Schemas


# pylint: disable=too-few-public-methods
class BO4EHighlighter(RegexHighlighter):
    """
    Custom highlighter for this CLI.
    """

    base_style = "bo4e."
    highlights: list[str | re.Pattern[str]] = [  # type: ignore[assignment]
        # This is typed as string in superclass, but apparently it works without problems when using re.Pattern.
        re.compile(r"\b(?P<bo4e_bo>BO)(?P<bo4e_4e>4E)\b", re.IGNORECASE),
        re.compile(r"\b(?:(?P<bo>bo)|(?P<com>com)|(?P<enum>enum))\b", re.IGNORECASE),
        re.compile(r"(?P<version>v?\d{6}\.\d+\.\d+(?:-rc\d*)?(?:\+dev\w+)?)"),
        re.compile(r"(?P<win_path>\b[a-zA-Z]:(?:\\[-\w._+]+)*\\)(?P<filename>[-\w._+]*)"),
        re.compile(r"\b(?P<json>JSON)\b", re.IGNORECASE),
    ]


def get_bo4e_schema_highlighter(schemas: Schemas) -> Highlighter:
    """
    Create a highlighter for the BO4E schemas. The highlighter will highlight all schema names according to
    their module (bo, com, enum).
    """
    bo_names = []
    com_names = []
    enum_names = []
    unmatched_names = []
    for schema in schemas:
        if schema.module[0] == "bo":
            bo_names.append(schema.name)
        elif schema.module[0] == "com":
            com_names.append(schema.name)
        elif schema.module[0] == "enum":
            enum_names.append(schema.name)
        else:
            unmatched_names.append(schema.name)

    class BO4ESchemaHighlighter(RegexHighlighter):
        """
        Highlighter for BO4E schemas. Highlights BO, COM and ENUM schemas.
        Also highlights unmatched schemas i.e. with unmatched module names.
        """

        base_style = "bo4e."
        highlights: list[str | re.Pattern[str]] = [  # type: ignore[assignment]
            re.compile(rf"(?:^|\s)(?P<bo>(?:\.\./|\./)*(?:bo/)?(?:{'|'.join(bo_names)})(?:\.json#?)?)(?:\s|$)"),
            re.compile(rf"(?:^|\s)(?P<com>(?:\.\./|\./)*(?:com/)?(?:{'|'.join(com_names)})(?:\.json#?)?)(?:\s|$)"),
            re.compile(rf"(?:^|\s)(?P<enum>(?:\.\./|\./)*(?:enum/)?(?:{'|'.join(enum_names)})(?:\.json#?)?)(?:\s|$)"),
            re.compile(
                rf"(?:^|\s)(?P<bo4e_4e>(?:\.\./|\./)*(?:\w+/)?(?:{'|'.join(unmatched_names)})(?:\.json#?)?)(?:\s|$)"
            ),
        ]

    return BO4ESchemaHighlighter()


class HighlighterMixer(Highlighter):
    """
    Mix multiple highlighters into one. They will be applied in the order they are passed to the constructor.
    """

    def __init__(self, *highlighters: Highlighter):
        self.highlighters = list(highlighters)

    def highlight(self, text: Text) -> None:
        """Highlight :class:`rich.text.Text` using regular expressions.

        Args:
            text (~Text): Text to highlight.
        """
        for highlighter in self.highlighters:
            highlighter.highlight(text)


class ColorPalette:
    """
    A color palette for the BO4E theme. Only use colors from this palette to ensure a consistent look.
    """

    MAIN = Color.parse("#8cc04d")
    SUB = Color.parse("#617d8b")
    ERROR = Color.parse("#e35b3a")

    BO = Color.parse("#b6d7a8")
    COM = Color.parse("#e0a86c")
    ENUM = Color.parse("#d1c358")

    MAIN_ACCENT = Color.parse("#b9ff66")
    SUB_ACCENT = Color.parse("#96c1d7")


STYLES = {
    **DEFAULT_STYLES,
    "warning": Style(color=ColorPalette.ERROR),
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
