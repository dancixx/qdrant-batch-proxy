# post.lua
wrk.method = "POST"
wrk.body   = '{"inputs":["What is Vector Search?", "Hello, world!"]}'
wrk.headers["Content-Type"] = "application/json"
