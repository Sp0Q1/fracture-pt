document.addEventListener('DOMContentLoaded', function () {
    // Clickable rows/cards: navigate to data-href on click
    document.querySelectorAll('[data-href]').forEach(function (el) {
        el.addEventListener('click', function () {
            window.location.href = el.getAttribute('data-href');
        });
    });

    // Delete buttons: uses data-delete-url and data-delete-redirect attributes
    document.querySelectorAll('[data-delete-url]').forEach(function (button) {
        button.addEventListener('click', function (event) {
            event.preventDefault();
            var deleteUrl = this.getAttribute('data-delete-url');
            var redirectTo = this.getAttribute('data-delete-redirect');
            if (confirm('Are you sure you want to delete this item?')) {
                var xhr = new XMLHttpRequest();
                xhr.open('DELETE', deleteUrl, true);
                xhr.onreadystatechange = function () {
                    if (xhr.readyState === 4 && xhr.status === 200) {
                        window.location.href = redirectTo;
                    }
                };
                xhr.send();
            }
        });
    });

    // Copy to clipboard: uses data-copy attribute
    document.querySelectorAll('[data-copy]').forEach(function (button) {
        button.addEventListener('click', function () {
            var text = this.getAttribute('data-copy');
            var btn = this;
            var original = btn.textContent;
            navigator.clipboard.writeText(text).then(function () {
                btn.textContent = 'Copied!';
                window.setTimeout(function () { btn.textContent = original; }, 1500);
            });
        });
    });

    // Session refresh: only runs when body has data-authenticated
    if (document.body.hasAttribute('data-authenticated')) {
        setInterval(function () {
            fetch('/api/auth/oidc/refresh').then(function (r) {
                if (!r.ok) {
                    document.body.innerHTML =
                        '<div class="text-center mt-6">' +
                        '<h2>Session expired</h2><p>You have been logged out.</p>' +
                        '<a href="/api/auth/oidc/authorize">Sign in again</a></div>';
                }
            });
        }, 12 * 60 * 1000);
    }
});
