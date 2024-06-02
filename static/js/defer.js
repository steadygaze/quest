// Convert a string to a similar username or URL slug.
function usernameify(str) {
  return str
    .normalize("NFKD")
    .toLowerCase()
    .replaceAll(/[^a-z0-9]/g, "");
}
