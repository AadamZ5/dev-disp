# thread-future

A simple future that spawns a thread, and resolves once the thread is complete. Has support
for custom cancelation token integration so that threads can be canceled if the future
is dropped. Also supports lazy thread creation until the first poll on the future.
