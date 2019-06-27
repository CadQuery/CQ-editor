from setuptools import setup, find_packages

setup(name='CQ-editor',
      version='0.1.0dev',
      packages=find_packages(),
      entry_points={
          'gui_scripts': [
              'cq-editor = cq_editor.__main__:main',
              'CQ-editor = cq_editor.__main__:main' 
          ]}
     )
