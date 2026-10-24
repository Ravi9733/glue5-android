cmake_minimum_required(VERSION 3.22.1)
project(xbox360prototype LANGUAGES CXX)

add_library(xbox360prototype SHARED native-lib.cpp)
target_compile_features(xbox360prototype PRIVATE cxx_std_17)
