
group "default" {
  targets = [
    "",
  ]
}
target "" {
  context    = "./"
  dockerfile = "./Dockerfile"
  tags       = [
    ":latest",
  ]
}
