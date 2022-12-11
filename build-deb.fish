#!/usr/bin/env fish

set -l _version "1.0.0"

# Aborts the installer and displays an error.
function abort -a message
  if test -n "$message"
    printf "%saborted: $message%s\n" (set_color -o red 2> /dev/null) (set_color normal 2> /dev/null) >&2
  else
    printf "%saborted%s\n" (set_color -o red 2> /dev/null) (set_color normal 2> /dev/null) >&2
  end

  exit 1
end


docker buildx build -t zero2prod .
  or die "failed to build the docker builder image"

set -l _id (docker create zero2prod)

docker cp "$_id:/app/target/debian/zero2prod_"$_version"_amd64.deb" .
