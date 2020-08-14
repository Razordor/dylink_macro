# Dylink Macro
## Description
This is an attribute macro that expands to dynamically linked code. All functions
declared are expanded into thunks. Vulkan and Opengl have specializations at the time of expansion.
## Crate Dependencies
The following is the minimum dependency required for macro expansion:
* `once_cell = "1.4.0"`
## Function Dependencies
The following are the minimum functions that need a definition:
```rs 
pub fn vkloader(&str, Context) -> *const c_void; // Vulkan specialization
pub fn glloader(&str) -> *const c_void; // Opengl specialization
pub fn loader(&str, &str) -> *const c_void; // Generalization
```