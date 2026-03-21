
void c() { int x = 42; }
void b() { c(); }
void a() { b(); }
  
int main() { a(); return 0; }
