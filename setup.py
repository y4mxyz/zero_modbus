from setuptools import setup, find_packages


setup(
    name='zero_modbus_pyclient',
    version='0.1.0',
    packages=find_packages(),
    requires=['pymodbus', 'pyserial'],
)