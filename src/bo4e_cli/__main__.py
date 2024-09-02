"""
This module is the entry point for the bo4e-cli. It is called when the bo4e-cli is executed.
"""

from bo4e_cli.commands.entry import app


def main() -> None:
    """Entry point function"""
    app()


if __name__ == "__main__":
    main()
