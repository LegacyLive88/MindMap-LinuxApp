<?php
/**
 * Public front door for a cPanel domain.
 *
 * nginx on WHM hands the request to Apache, Apache runs this script, and
 * Guzzle forwards it to the Next.js app bound to 127.0.0.1. The browser and
 * the desktop program both use the public domain. They never talk to the
 * Node port directly.
 */

declare(strict_types=1);

$autoload = dirname(__DIR__) . '/vendor/autoload.php';
if (!is_file($autoload)) {
    http_response_code(500);
    header('Content-Type: text/plain; charset=utf-8');
    echo "Run composer install in the cpanel directory so Guzzle is available.\n";
    exit;
}

require $autoload;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\GuzzleException;

$upstream = upstream();
$method = $_SERVER['REQUEST_METHOD'] ?? 'GET';
$allowed = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];
if (!in_array($method, $allowed, true)) {
    http_response_code(405);
    header('Content-Type: text/plain; charset=utf-8');
    echo "Method not allowed.\n";
    exit;
}

$uri = $_SERVER['REQUEST_URI'] ?? '/';
$body = file_get_contents('php://input');
$headers = forwarded_headers();
$options = ['headers' => $headers];
if (!in_array($method, ['GET', 'HEAD'], true)) {
    $options['body'] = $body === false ? '' : $body;
}

$client = new Client([
    'base_uri' => $upstream,
    'http_errors' => false,
    'allow_redirects' => false,
    'timeout' => 60,
    'connect_timeout' => 5,
]);

try {
    $response = $client->request($method, $uri, $options);
} catch (GuzzleException $error) {
    http_response_code(502);
    header('Content-Type: text/plain; charset=utf-8');
    echo "The MindMap cloud is not running on this server.\n";
    exit;
}

http_response_code($response->getStatusCode());
$pass = ['content-type', 'cache-control', 'location', 'set-cookie', 'www-authenticate'];
foreach ($response->getHeaders() as $name => $values) {
    if (!in_array(strtolower($name), $pass, true)) {
        continue;
    }
    foreach ($values as $value) {
        header($name . ': ' . $value, false);
    }
}
echo $response->getBody()->getContents();

function upstream(): string
{
    $fromEnv = getenv('MINDMAP_UPSTREAM');
    if (is_string($fromEnv) && $fromEnv !== '') {
        return rtrim($fromEnv, '/');
    }
    $file = dirname(__DIR__) . '/upstream.txt';
    if (is_file($file)) {
        $text = trim((string) file_get_contents($file));
        if ($text !== '') {
            return rtrim($text, '/');
        }
    }
    return 'http://127.0.0.1:3000';
}

function forwarded_headers(): array
{
    $headers = ['Accept' => $_SERVER['HTTP_ACCEPT'] ?? 'application/json'];
    $authorization = $_SERVER['HTTP_AUTHORIZATION']
        ?? $_SERVER['REDIRECT_HTTP_AUTHORIZATION']
        ?? '';
    if ($authorization !== '') {
        $headers['Authorization'] = $authorization;
    }
    $copy = [
        'CONTENT_TYPE' => 'Content-Type',
        'HTTP_COOKIE' => 'Cookie',
        'HTTP_USER_AGENT' => 'User-Agent',
    ];
    foreach ($copy as $serverKey => $headerName) {
        if (!empty($_SERVER[$serverKey])) {
            $headers[$headerName] = $_SERVER[$serverKey];
        }
    }
    $proto = $_SERVER['HTTP_X_FORWARDED_PROTO'] ?? ((!empty($_SERVER['HTTPS']) && $_SERVER['HTTPS'] !== 'off') ? 'https' : 'http');
    $headers['X-Forwarded-Proto'] = $proto;
    $remote = $_SERVER['REMOTE_ADDR'] ?? '';
    if ($remote !== '') {
        $existing = $_SERVER['HTTP_X_FORWARDED_FOR'] ?? '';
        $headers['X-Forwarded-For'] = $existing !== '' ? $existing : $remote;
    }
    return $headers;
}
