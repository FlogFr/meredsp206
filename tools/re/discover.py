# Microchip announce discovery: prints every reply to "Discovery: Who is out there?" on UDP 30303
import socket, sys, time
targets = sys.argv[1:] or ['255.255.255.255']
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
s.bind(('', 0)); s.settimeout(0.5)
for t in targets:
    s.sendto(b'Discovery: Who is out there?\x00', (t, 30303))
end = time.time() + 3
while time.time() < end:
    try:
        d, a = s.recvfrom(2048); print(a, d)
    except socket.timeout:
        pass
