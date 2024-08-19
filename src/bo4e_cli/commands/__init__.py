from .diff import sub_app_diff
from .edit import edit
from .entry import app
from .generate import generate
from .pull import pull

app.add_typer(sub_app_diff, name="diff")
