from glob import glob
from subprocess import call
from os import remove

TEMPLATE = \
'''<RCC>
  <qresource prefix="/images">
    {}
  </qresource>
</RCC>'''

ITEM_TEMPLATE = '<file>{}</file>'

QRC_OUT = 'icons.qrc'
RES_OUT = 'src/icons_res.py'
TOOL = 'pyrcc5'

items = []

for i in glob('icons/*.svg'):
    items.append(ITEM_TEMPLATE.format(i))
    
    
qrc_text = TEMPLATE.format('\n'.join(items))

with open(QRC_OUT,'w') as f:
    f.write(qrc_text)
    
call([TOOL,QRC_OUT,'-o',RES_OUT])
remove(QRC_OUT)
