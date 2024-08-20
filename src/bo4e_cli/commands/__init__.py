"""
This package contains the commands for the bo4e-cli.
"""

from .diff import diff_matrix, diff_schemas, diff_version_bump_type, sub_app_diff
from .edit import edit
from .entry import app
from .generate import generate
from .pull import pull

app.add_typer(sub_app_diff, name="diff")
