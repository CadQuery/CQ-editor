import codecs
import os.path

from setuptools import setup, find_packages

def read(rel_path):
    here = os.path.abspath(os.path.dirname(__file__))
    with codecs.open(os.path.join(here, rel_path), 'r') as fp:
        return fp.read()

def get_version(rel_path):
    for line in read(rel_path).splitlines():
        if line.startswith('__version__'):
            delim = '"' if '"' in line else "'"
            return line.split(delim)[1]
    else:
        raise RuntimeError("Unable to find version string.")

setup(
    name="CQ-editor",
    version=get_version("cq_editor/_version.py"),
    packages=find_packages(),
    entry_points={
        "gui_scripts": [
            "cq-editor = cq_editor.__main__:main",
            "CQ-editor = cq_editor.__main__:main",
        ]
    },
    python_requires=">=3.8,<3.11",
    install_requires=[
        "logbook>=1",
        "ipython==8.4.0",
        "jedi==0.17.2",
        "path>=16",
        "PyQt5>=5",
        "requests>=2,<3",
        "spyder>=5,<6",
        "pyqtgraph==0.12.4",
    ],
)
