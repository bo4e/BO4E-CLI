"""
Monkey patching the typer.rich_utils module to use the BO4E theme and highlighter.
"""

import typer.rich_utils
from rich.console import Console
from rich.style import Style

from . import style

# pylint: disable=protected-access
_old_get_rich_console = typer.rich_utils._get_rich_console


def _get_rich_console_monkey_patch(stderr: bool = False) -> Console:
    console = _old_get_rich_console(stderr)
    console.push_theme(style.BO4ETheme)
    return console


typer.rich_utils._get_rich_console = _get_rich_console_monkey_patch
typer.rich_utils.highlighter = style.HighlighterMixer(  # type: ignore[assignment]
    typer.rich_utils.highlighter, style.BO4EHighlighter()
)
typer.rich_utils.negative_highlighter = style.HighlighterMixer(  # type: ignore[assignment]
    typer.rich_utils.negative_highlighter, style.BO4EHighlighter()
)

typer.rich_utils.STYLE_DEPRECATED = Style(color=style.ColorPalette.ERROR)  # type: ignore[assignment]
typer.rich_utils.STYLE_HELPTEXT_FIRST_LINE = Style(bold=True)  # type: ignore[assignment]
typer.rich_utils.STYLE_OPTION_ENVVAR = Style(color=style.ColorPalette.ENUM, dim=True)  # type: ignore[assignment]
typer.rich_utils.STYLE_REQUIRED_SHORT = Style(color=style.ColorPalette.ERROR)  # type: ignore[assignment]
typer.rich_utils.STYLE_REQUIRED_LONG = Style(color=style.ColorPalette.ERROR, dim=True)  # type: ignore[assignment]
typer.rich_utils.STYLE_ERRORS_PANEL_BORDER = Style(color=style.ColorPalette.ERROR)  # type: ignore[assignment]
typer.rich_utils.STYLE_ABORTED = Style(color=style.ColorPalette.ERROR)  # type: ignore[assignment]
