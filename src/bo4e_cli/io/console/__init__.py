"""
This package provides a global Console instance that can be used to print messages to the console.
"""

from .console import CONSOLE
from .monkey import *  # Load monkey patches

__all__ = ["CONSOLE"]
