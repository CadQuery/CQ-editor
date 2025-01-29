import codecs
import os.path

from setuptools import setup, find_packages

<<<<<<< HEAD

def read(rel_path):
    here = os.path.abspath(os.path.dirname(__file__))
    with codecs.open(os.path.join(here, rel_path), "r") as fp:
        return fp.read()


def get_version(rel_path):
    for line in read(rel_path).splitlines():
        if line.startswith("__version__"):
=======
def read(rel_path):
    here = os.path.abspath(os.path.dirname(__file__))
    with codecs.open(os.path.join(here, rel_path), 'r') as fp:
        return fp.read()

def get_version(rel_path):
    for line in read(rel_path).splitlines():
        if line.startswith('__version__'):
>>>>>>> d746c8e8e6d80f53dc931ddc4910d4c791d7218b
            delim = '"' if '"' in line else "'"
            return line.split(delim)[1]
    else:
        raise RuntimeError("Unable to find version string.")

<<<<<<< HEAD

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
    python_requires=">=3.10,<3.14",
    install_requires=[
        "logbook>=1",
        "ipython",
        "path>=16",
        "PyQt5>=5",
        "requests>=2,<3",
        "spyder>=5,<6",
        "pyqtgraph",
        "numpy >= 2, <3",
    ],
)
=======
setup(name='CQ-editor',
      version=get_version('cq_editor/_version.py'),
      packages=find_packages(),
      entry_points={
          'gui_scripts': [
              'cq-editor = cq_editor.__main__:main',
              'CQ-editor = cq_editor.__main__:main' 
          ]}
     )
>>>>>>> d746c8e8e6d80f53dc931ddc4910d4c791d7218b
