A few decision and constraints which documents the history of the API changes.

### Naming

The API tries to follow the likes of 'modern' Khronos APIs in terms of naming stuffs and levels of abstraction. `Instance` as central entry point, `PhysicalDevice` for querying device information and `Device` as the logical instantiation of a physical device.
The motivation is to fit along nicely with other low level components like Vulkan.

### Unsafety and Typesafety

Based on my own experience, trying to write wrappers and designing nice Rust-ic APIs on top can be very challenging and time consuming. This lead to the conclusion that these should be decoupled by providing an minimal & unsafe abstraction layer for the platform APIs and design sth on top based on the user desires. This library does the former part (unsafe abstraction layer).

### Callback vs Polling

Platform APIs work different, some provide a polling based approach (e.g WASAPI), others are purely callback based like OpenSL and a few support both (e.g AAudio). On related aspect of this the executor, which does the polling and callback invocation.
For callback based approaches the platform takes care of the executor already (with options for configuration?). For polling the user needs to take care of it, usually in a separated high priority thread. To give the most flexibility to user we decided on exposing either callback or polling based on the platform API. In a further step we want to provide initialization functions for executors.

### Constraints

- not possible to query all supported formats from a physical device (WASAPI)
- AAudio requires callback to be set when opening the device
- Exact stream properties only known after creation
- Format selection is somewhat tricky, cpal's default format function difficult to support on all platforms
