// Convert a string to a similar username or URL slug.
function usernameify(str) {
  return str
    .normalize("NFKD")
    .toLowerCase()
    .replaceAll(/[^a-z0-9]/g, "");
}

function check_username(event) {
  const username = event.detail.requestConfig.parameters.username;
  if (globalThis.username && globalThis.username === username) {
    // Prevent double-triggering in some circumstances.
    event.preventDefault(); // Cancel AJAX.
    return;
  }
  globalThis.username = username;
  if (username.length < 3) {
    event.preventDefault(); // Cancel AJAX.
    document.getElementById("username-validation-target").innerHTML =
      "<span class='text-amber-600'>Username too short</span>";
  } else if (/[^a-z0-9]/.test(username)) {
    event.preventDefault(); // Cancel AJAX.
    document.getElementById("username-validation-target").innerHTML =
      "<span class='text-amber-600'>Username contains unallowed characters</span>";
  } else if (/^[^a-z]/.test(username)) {
    event.preventDefault(); // Cancel AJAX.
    document.getElementById("username-validation-target").innerHTML =
      "<span class='text-amber-600'>Username must start with a letter (not a number)</span>";
  }
}
