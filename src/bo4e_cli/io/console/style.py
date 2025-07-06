"""
This module contains the style definitions and highlighters for the BO4E CLI.
"""

import bisect
import re
from collections import defaultdict
from collections.abc import Callable
from typing import Iterable, TypeAlias, cast

from rich.color import Color
from rich.highlighter import Highlighter
from rich.style import Style
from rich.text import Text
from rich.theme import Theme

from bo4e_cli.models.meta import Schemas
from bo4e_cli.models.schema import SchemaRootObject, SchemaRootStrEnum
from bo4e_cli.utils.iterator import zip_cycle

Priority: TypeAlias = int
Pattern: TypeAlias = re.Pattern[str] | str


# pylint: disable=too-few-public-methods


class _Highlighter:
    """
    This is an internal class to make debugging easier. When looking at the executor functions of a HighlighterMixer
    (see below), the name of the highlighter is displayed in the repr of the executor.
    """

    def __init__(self, func: Callable[[Text], None], name: str):
        self.func = func
        self.name = name

    def __call__(self, text: Text) -> None:
        self.func(text)

    def __repr__(self) -> str:
        return f"<_Highlighter {self.name}>"


class RegexPriorityHighlighter(Highlighter):
    """
    Applies highlighting from a list of regular expressions in the order of lowest priority to highest to enable
    styles to be overwritten.
    """

    def __init__(
        self, *highlights: Pattern | tuple[Pattern, Priority], base_style: str = "", default_priority: Priority = 100
    ):
        self.highlights = cast(
            tuple[tuple[Pattern, Priority]],
            tuple(
                sorted(
                    (
                        highlight_ if isinstance(highlight_, tuple) else (highlight_, default_priority)
                        for highlight_ in highlights
                    ),
                    key=lambda x: x[1],
                )
            ),
        )
        self.base_style = base_style
        self.default_priority = default_priority

    def highlight(self, text: Text) -> None:
        """Highlight :class:`rich.text.Text` using regular expressions.

        Args:
            text (~Text): Text to highlight.

        """
        for re_highlight, _ in self.highlights:
            text.highlight_regex(re_highlight, style_prefix=self.base_style)
            # previously mypy correctly reported an error here (arg-type).
            # mypy is right, but the function in rich.Text also works with re.Pattern and is therefore wrongly annotated
            # Since upgrade from mypy v1.11.2 -> v1.12.0 this error is not reported anymore (False-negative).
            # See PR https://github.com/bo4e/BO4E-CLI/pull/32
            # Maybe this will be fixed in a future version of mypy then you will need the type-ignore again.


class BO4EHighlighter(RegexPriorityHighlighter):
    """
    Custom highlighter for this CLI.
    """

    def __init__(self) -> None:
        super().__init__(
            (re.compile(r"\b(?P<bo4e_bo>BO)(?P<bo4e_4e>4E)\b", re.IGNORECASE), 90),
            (re.compile(r"\b(?:(?P<bo>bo)|(?P<com>com)|(?P<enum>enum))\b", re.IGNORECASE), 50),
            (re.compile(r"\b(?P<json>JSON)\b", re.IGNORECASE), 50),
            (re.compile(r"(?P<version>v?\d{6}\.\d+\.\d+(?:-rc\d*)?(?:\+dev\w+)?)"), 120),
            (re.compile(r"(?P<win_path>\b[a-zA-Z]:(?:\\[-\w._+]+)*\\)(?P<filename>[-\w._+]*)"), 120),
            base_style="bo4e.",
        )


def get_bo4e_schema_highlighter(schemas: Schemas, match_fields: bool = False) -> Highlighter:
    """
    Create a highlighter for the BO4E schemas. The highlighter will highlight all schema names according to
    their module (bo, com, enum).
    """
    names = defaultdict(set)
    field_names: defaultdict[str, set[str]] = defaultdict(set)
    for schema in schemas:
        if schema.module[0] in ("bo", "com", "enum"):
            names[schema.module[0]].add(schema.name)
            if match_fields and isinstance(schema.schema_parsed, SchemaRootObject):
                field_names[schema.module[0]].update(schema.schema_parsed.properties.keys())
            elif match_fields and isinstance(schema.schema_parsed, SchemaRootStrEnum):
                field_names[schema.module[0]].update(schema.schema_parsed.enum)
        else:
            # Unmatched schemas
            names["bo4e_4e"].add(schema.name)
            if match_fields and isinstance(schema.schema_parsed, SchemaRootObject):
                field_names["bo4e_4e"].update(schema.schema_parsed.properties.keys())

    names_regex = {module: f"(?:{'|'.join(cls_names)})" for module, cls_names in names.items()}
    field_names_regex = {module: f"(?:{'|'.join(mod_field_names)})" for module, mod_field_names in field_names.items()}

    path_sep = r"[\\/]"
    regex_rel_path = {
        mod: re.compile(rf"\b(?P<{mod}>(?:\.\.{path_sep}{mod}{path_sep}|\.?{path_sep})?{mod_regex}(?:\.json#?)?)\b")
        for mod, mod_regex in names_regex.items()
        if mod != "bo4e_4e"
    }
    regex_rel_path["bo4e_4e"] = re.compile(
        rf"\b(?P<bo4e_4e>(?:\.\.{path_sep}|\.?{path_sep})?{names_regex['bo4e_4e']}(?:\.json#?)?)\b"
    )

    if match_fields:
        regex_mod_path = {
            mod: re.compile(rf"\b(?P<{mod}>(?:{mod}\.)?{mod_regex}(?:\.{field_names_regex[mod]})?)\b")
            for mod, mod_regex in names_regex.items()
            if mod != "bo4e_4e"
        }
        regex_mod_path["bo4e_4e"] = re.compile(
            rf"\b(?P<bo4e_4e>{names_regex['bo4e_4e']}(?:\.{field_names_regex['bo4e_4e']})?)\b"
        )
    else:
        # This is for performance reasons. If we don't need to match fields, we can use a simpler regex.
        regex_mod_path = {
            mod: re.compile(rf"\b(?P<{mod}>(?:{mod}\.)?{mod_regex})\b")
            for mod, mod_regex in names_regex.items()
            if mod != "bo4e_4e"
        }
        regex_mod_path["bo4e_4e"] = re.compile(rf"\b(?P<bo4e_4e>{names_regex['bo4e_4e']})\b")

    class BO4ESchemaHighlighter(RegexPriorityHighlighter):
        """
        Highlighter for BO4E schemas. Highlights BO, COM and ENUM schemas.
        Also highlights unmatched schemas i.e. with unmatched module names.
        """

        def __init__(self) -> None:
            super().__init__(
                *zip_cycle(regex_rel_path.values(), els_to_cycle=(60,)),
                *zip_cycle(regex_mod_path.values(), els_to_cycle=(60,)),
                base_style="bo4e.",
            )

    return BO4ESchemaHighlighter()


class HighlighterMixer(Highlighter):
    """
    Mix multiple highlighters into one.
    They will be applied in the order of priority. If a highlighter is an instance of RegexPriorityHighlighter,
    instead of executing the highlighter, the patterns and its priorities will be used and sorted in an internal list.
    """

    def __init__(self, *highlighters: Highlighter | tuple[Highlighter, Priority], default_priority: Priority = 100):
        self._executors: list[tuple[Callable[[Text], None], Priority]] = list(
            sorted(
                self._get_executors(
                    *(
                        highlighter if isinstance(highlighter, tuple) else (highlighter, default_priority)
                        for highlighter in highlighters
                    )
                ),
                key=lambda x: x[1],
            )
        )
        self._default_priority = default_priority

    @staticmethod
    def _get_executor(non_priority_highlighter: Highlighter) -> Callable[[Text], None]:
        assert not isinstance(non_priority_highlighter, RegexPriorityHighlighter)
        return _Highlighter(non_priority_highlighter.highlight, type(non_priority_highlighter).__name__)

    @staticmethod
    def _get_executor_from_regex(pattern: Pattern, base_style: str) -> Callable[[Text], None]:
        return _Highlighter(
            lambda text: text.highlight_regex(pattern, style_prefix=base_style),  # type: ignore[arg-type]
            pattern.pattern if isinstance(pattern, re.Pattern) else pattern,
        )

    @staticmethod
    def _get_executors_from_regex_highlighter(
        highlighter: RegexPriorityHighlighter,
    ) -> Iterable[tuple[Callable[[Text], None], Priority]]:
        for pattern, priority in highlighter.highlights:
            yield HighlighterMixer._get_executor_from_regex(pattern, highlighter.base_style), priority

    @staticmethod
    def _get_executors(
        *highlighters_and_priorities: tuple[Highlighter, Priority]
    ) -> Iterable[tuple[Callable[[Text], None], Priority]]:
        for highlighter_and_priority in highlighters_and_priorities:
            highlighter = highlighter_and_priority[0]
            if isinstance(highlighter, RegexPriorityHighlighter):
                yield from HighlighterMixer._get_executors_from_regex_highlighter(highlighter)
            else:
                yield HighlighterMixer._get_executor(highlighter), highlighter_and_priority[1]

    def add(self, highlighter: Highlighter, priority: Priority | None = None) -> None:
        """
        Add a highlighter to the mixer.
        """
        if priority is None:
            priority = self._default_priority
        for executor, prio in self._get_executors((highlighter, priority)):
            bisect.insort(self._executors, (executor, prio), key=lambda x: x[1])

    def highlight(self, text: Text) -> None:
        """Highlight :class:`rich.text.Text` using regular expressions.

        Args:
            text (~Text): Text to highlight.
        """
        for executor, _ in self._executors:
            executor(text)


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
    "warning": Style(color=ColorPalette.ERROR),
    "bo4e.bo4e_bo": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.bo4e_4e": Style(color=ColorPalette.SUB, bold=True),
    "bo4e.bo": Style(color=ColorPalette.BO, bold=True),
    "bo4e.com": Style(color=ColorPalette.COM, bold=True),
    "bo4e.enum": Style(color=ColorPalette.ENUM, bold=True),
    "bo4e.field": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.version": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.win_path": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.filename": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.json": Style(color=ColorPalette.COM),
    "bo4e.table.title": Style(color=ColorPalette.MAIN, bold=True),
    "bo4e.table.header": Style(color=ColorPalette.MAIN),
    "bo4e.table.row1": Style(color=ColorPalette.SUB),
    "bo4e.table.row2": Style(color=ColorPalette.SUB, dim=True),
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
