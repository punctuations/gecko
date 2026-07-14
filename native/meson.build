project('gecko-runtime', 'c',
  version: '0.0.1',
  default_options: [
    'c_std=c11',
    'default_library=static',
    'warning_level=2',
  ])

inc = include_directories('include')

windows = host_machine.system() == 'windows'

gecko_runtime = static_library('gecko_runtime',
  sources: files(
    'src/value.c',
    'src/heap.c',
    'src/gc.c',
    'src/code.c',
    'src/setae.c',
    'src/builtins.c',
  ),
  include_directories: inc,
  name_prefix: windows ? '' : 'lib',
  name_suffix: windows ? 'lib' : 'a',
  install: false)
